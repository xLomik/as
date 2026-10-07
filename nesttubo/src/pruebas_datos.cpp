// Pruebas de la carpeta de datos (datos.cpp) bajo wine. Deben terminar en "FALLOS: 0".
//
//   ./compilar.sh datos     prepara la unidad de red N: en wine, compila y corre esto
//
// Recibe la carpeta que hace de servidor (la que apunta N:) en forma Z:\... para
// poder "desconectar" la red moviendola. Borra los datos de NestTubo del prefijo
// de wine: no correr en un equipo de verdad.
#include <windows.h>
#include <shellapi.h>

#include <cstdio>
#include <memory>
#include <string>
#include <vector>

#include "datos.h"

using namespace nt;
using namespace datos;

static int fallos = 0;

static void check(bool ok, const std::string& desc) {
    std::printf("%s %s\n", ok ? "OK " : "MAL", desc.c_str());
    std::fflush(stdout);
    if (!ok) fallos++;
}

static std::string u(const std::wstring& s) { return U(s); }

static void borrar_arbol(const std::wstring& c) {
    std::wstring doble = c + L'\0';
    SHFILEOPSTRUCTW op{};
    op.wFunc = FO_DELETE;
    op.pFrom = doble.c_str();
    op.fFlags = FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT;
    SHFileOperationW(&op);
}

static std::string leer(const std::wstring& r) {
    std::string d;
    leer_archivo(r, d);
    return d;
}

static bool escribir(const std::wstring& r, const std::string& d) { return escribir_archivo(r, d); }

// Informes del hilo
static std::unique_ptr<InformePasada> ultima_pasada;
static std::unique_ptr<InformeExaminar> ultimo_examen;

static LRESULT CALLBACK Proc(HWND h, UINT m, WPARAM w, LPARAM l) {
    if (m == WM_APP_RED) {
        if (w == INFORME_PASADA) ultima_pasada.reset((InformePasada*)l);
        else ultimo_examen.reset((InformeExaminar*)l);
        return 0;
    }
    return DefWindowProcW(h, m, w, l);
}

static bool esperar(bool examen, DWORD ms) {
    DWORD fin = GetTickCount() + ms;
    while ((int)(fin - GetTickCount()) > 0) {
        MSG msg;
        while (PeekMessageW(&msg, nullptr, 0, 0, PM_REMOVE)) DispatchMessageW(&msg);
        if (examen ? (bool)ultimo_examen : (bool)ultima_pasada) return true;
        Sleep(20);
    }
    return false;
}

// Pide una pasada y devuelve su informe (compilar.sh apaga las pasadas solas).
static InformePasada* pasada(const std::wstring& archivo = L"") {
    ultima_pasada.reset();
    unsigned long long n = pedir_pasada(archivo);
    if (!esperar_pasada(n, 20000)) return nullptr;
    esperar(false, 20000);
    return ultima_pasada.get();
}

static Conexion conexion(const InformePasada* p, const std::wstring& r) {
    if (p)
        for (const auto& x : p->raices)
            if (clave_ruta(x.first) == clave_ruta(r)) return x.second;
    return Conexion::desconocida;
}

static std::string nombres(const std::vector<PerfilTxt>& v) {
    std::string r;
    for (const auto& p : v) r += (r.empty() ? "" : ",") + p.nombre + "=" + p.barra;
    return r;
}

static PerfilTxt P(const std::string& n, const std::string& barra) {
    PerfilTxt p = perfil_nuevo(n);
    p.barra = barra;
    return p;
}

// Lo que la ventana recuerda del archivo copiado (app.sello).
static Sello sello_copiado(const InformePasada* p, const std::wstring& destino) {
    if (p)
        for (const auto& c : p->copiados)
            if (clave_ruta(c.destino) == clave_ruta(destino) && clave_ruta(c.escrito) == clave_ruta(destino)) return c.sello;
    return Sello{};
}

static std::wstring escrito(const InformePasada* p, const std::wstring& destino) {
    if (p)
        for (const auto& c : p->copiados)
            if (clave_ruta(c.destino) == clave_ruta(destino)) return c.escrito;
    return L"";
}

static std::vector<PerfilTxt> catalogo_en(const std::wstring& ruta) {
    Trabajo t;
    std::string e;
    de_texto(leer(ruta), t, e);
    return t.perfiles;
}

int wmain(int argc, wchar_t** argv) {
    if (argc < 2) {
        std::puts("uso: pruebas_datos Z:\\ruta\\del\\servidor   (la carpeta a la que apunta N:)");
        return 2;
    }
    const std::wstring srv = argv[1], srv_off = srv + L".off";
    const std::wstring N = L"N:\\datos";
    // estado limpio
    borrar_arbol(carpeta_local());
    DeleteFileW((carpeta_config() + L"\\config.ini").c_str());
    DeleteFileW((carpeta_config() + L"\\perfiles.ntb").c_str());
    if (existe(srv_off)) MoveFileW(srv_off.c_str(), srv.c_str());
    borrar_arbol(srv + L"\\datos");
    CreateDirectoryW((srv + L"\\datos").c_str(), nullptr);
    CreateDirectoryW(carpeta_local().c_str(), nullptr);

    std::puts("== rutas");
    check(es_remota(L"N:\\datos\\a.ntb"), "N: es una unidad de red (si falla, wine no la ve como de red)");
    check(!es_remota(L"C:\\x\\a.ntb") && !es_remota(L"\\\\?\\C:\\x") && es_remota(L"\\\\srv\\rec\\a.ntb"),
          "C:\\ no es de red; \\\\?\\ tampoco; \\\\srv\\rec si");
    check(es_remota(L"Q:\\x\\a.ntb"), "una letra que no existe se trata como de red: lo guardado espera a que vuelva");
    check(raiz(L"\\\\srv\\rec\\a\\b.ntb") == L"\\\\srv\\rec" && raiz(L"n:\\x") == L"N:\\" && raiz(L"x\\y").empty() &&
              raiz(L"\\\\srv").empty(),
          "raices: \\\\srv\\rec, N:\\, nada en relativas ni en \\\\srv solo");
    std::wstring pe = ruta_en_espera(L"N:\\datos\\Cliente A\\Pedido 1.ntb");
    check(pe == carpeta_espera() + L"\\N\\datos\\Cliente A\\Pedido 1.ntb", "ruta en espera: " + u(pe));
    std::wstring d;
    bool vuelta = destino_en_espera(pe, d) && d == L"N:\\datos\\Cliente A\\Pedido 1.ntb";
    check(vuelta, "y de vuelta: " + u(d));
    check(carpeta_en_espera(L"N:\\datos\\") == carpeta_espera() + L"\\N\\datos", "carpeta en espera: " + u(carpeta_en_espera(L"N:\\datos\\")));
    check(carpeta_de(L"C:\\a.ntb") == L"C:\\" && carpeta_de(L"N:\\d\\a.ntb") == L"N:\\d" && nombre_de(L"N:\\d\\a.ntb") == L"a.ntb",
          "carpeta_de y nombre_de");

    std::puts("== lectura y escritura");
    std::string datos_leidos;
    DWORD cod = 0;
    check(leer_archivo(N + L"\\no_hay.ntb", datos_leidos, &cod) == Lectura::no_existe, "archivo que no existe: no_existe");
    check(leer_archivo(N + L"\\no_hay\\x.ntb", datos_leidos, &cod) == Lectura::error, "carpeta que no existe: error, no vacio");
    CreateDirectoryW((N + L"\\carpeta.ntb").c_str(), nullptr);
    check(leer_archivo(N + L"\\carpeta.ntb", datos_leidos, &cod) == Lectura::error, "una carpeta con nombre de archivo: error");
    RemoveDirectoryW((N + L"\\carpeta.ntb").c_str());
    check(escribir(N + L"\\x.ntb", "hola") && leer(N + L"\\x.ntb") == "hola", "escribir y leer en N:");
    WIN32_FIND_DATAW fd;
    HANDLE hf = FindFirstFileW((N + L"\\*.tmp").c_str(), &fd);
    check(hf == INVALID_HANDLE_VALUE, "no quedan temporales");
    if (hf != INVALID_HANDLE_VALUE) FindClose(hf);
    DeleteFileW((N + L"\\x.ntb").c_str());
    check(crear_carpetas(carpeta_local() + L"\\a\\b\\c") && existe(carpeta_local() + L"\\a\\b\\c"), "crear_carpetas anidadas");
    borrar_arbol(carpeta_local() + L"\\a");

    // ventana invisible para recibir los informes del hilo
    WNDCLASSW wc{};
    wc.lpfnWndProc = Proc;
    wc.hInstance = GetModuleHandleW(nullptr);
    wc.lpszClassName = L"PruebasDatos";
    RegisterClassW(&wc);
    HWND w = CreateWindowW(L"PruebasDatos", L"", 0, 0, 0, 0, 0, HWND_MESSAGE, nullptr, wc.hInstance, nullptr);
    iniciar_hilo(w);

    std::puts("== trabajos: con red");
    const std::wstring destino = N + L"\\Cliente A\\Pedido 1.ntb";
    bool en_espera = false;
    std::wstring error;
    Sello sello, base;
    bool guardado = guardar_trabajo(destino, "version 1", sello_nuevo(), en_espera, sello, error);
    check(guardado && en_espera && leer(pe) == "version 1" && !existe(destino), "Guardar en red: primero queda en este equipo " + u(error));
    std::wstring a_leer = version_a_abrir(destino, d, en_espera, base);
    check(a_leer == pe && d == destino && en_espera && base.conocido && !base.existe,
          "Abrir el destino lee la version en espera, con su base (trabajo nuevo)");
    a_leer = version_a_abrir(pe, d, en_espera, base);
    check(a_leer == pe && d == destino && en_espera, "Abrir el archivo en espera: el destino es el de red");
    InformePasada* p = pasada(destino);
    check(p && leer(destino) == "version 1" && !existe(pe) && !existe(pe + L".ntbase") && p->copiados.size() == 1 &&
              p->trabajos_en_espera == 0,
          "la pasada lo copia y borra el de este equipo y su base");
    sello = sello_copiado(p, destino);
    check(mismo_archivo(sello, sello_de(destino)), "el informe trae como quedo el archivo copiado");
    check(!existe(carpeta_espera() + L"\\N\\datos\\Cliente A") && !existe(carpeta_espera() + L"\\N"),
          "y borra las carpetas vacias de la espera");
    check(conexion(p, L"N:\\") == Conexion::en_linea, "N: en linea");
    a_leer = version_a_abrir(destino, d, en_espera, base);
    check(a_leer == destino && !en_espera, "sin version en espera, Abrir lee el destino");

    std::puts("== trabajos: sin red");
    check(MoveFileW(srv.c_str(), srv_off.c_str()), "desconectar la red (mover la carpeta del servidor)");
    check(guardar_trabajo(destino, "version 2", sello, en_espera, sello, error) && en_espera, "Guardar sin red funciona igual");
    const std::wstring otro = N + L"\\Cliente B\\Pedido 1.ntb";
    Sello sello_otro;
    check(guardar_trabajo(otro, "cliente B", sello_nuevo(), en_espera, sello_otro, error) && en_espera && ruta_en_espera(otro) != pe,
          "dos 'Pedido 1.ntb' de carpetas distintas: dos archivos en espera");
    p = pasada(destino);
    check(p && conexion(p, L"N:\\") == Conexion::sin_conexion && p->trabajos_en_espera == 2 && p->fallas.empty() && p->copiados.empty(),
          "sin red: N: sin conexion, 2 en espera y ningun error real");
    check(p && p->esperando_red.size() == 1 && clave_ruta(p->esperando_red[0].first) == clave_ruta(L"N:\\") && p->esperando_red[0].second == 2,
          "el informe dice que los 2 esperan a N:");
    check(MoveFileW(srv_off.c_str(), srv.c_str()), "reconectar");
    p = pasada(destino);
    check(p && leer(destino) == "version 2" && leer(otro) == "cliente B" && p->trabajos_en_espera == 0 && !existe(carpeta_espera() + L"\\N"),
          "al volver la red se copian los dos, cada uno a su carpeta (la base coincide: se reemplaza)");
    check(p && p->esperando_red.empty(), "y ya nada espera");
    sello = sello_copiado(p, destino);

    std::puts("== trabajos: guardar otra vez con red");
    check(guardar_trabajo(destino, "version 3", sello, en_espera, sello, error), "guardar version 3");
    p = pasada(destino);
    check(p && leer(destino) == "version 3", "version 3 copiada encima de la 2");
    sello = sello_copiado(p, destino);

    std::puts("== trabajos: no pisar otro archivo de la red");
    {
        // un trabajo nuevo guardado sin conexion con un nombre que en la red ya es otro trabajo
        Sello s;
        check(guardar_trabajo(destino, "otro pedido", sello_nuevo(), en_espera, s, error) && en_espera, "guardar un trabajo nuevo con el mismo nombre");
        p = pasada(destino);
        const std::wstring al_lado = N + L"\\Cliente A\\Pedido 1 (guardado sin conexión).ntb";
        check(p && leer(destino) == "version 3" && leer(al_lado) == "otro pedido" && clave_ruta(escrito(p, destino)) == clave_ruta(al_lado) &&
                  !existe(pe) && p->trabajos_en_espera == 0,
              "no reemplaza el de la red: queda al lado como '(guardado sin conexión)'");
        // el archivo de la red cambio despues de la base (otro equipo, u otro programa)
        check(guardar_trabajo(destino, "version 4 sin red", sello, en_espera, s, error) && en_espera, "guardar con la base de la version 3");
        Sleep(50);
        escribir(destino, "cambiado en la red mientras tanto");
        p = pasada(destino);
        const std::wstring al_lado2 = N + L"\\Cliente A\\Pedido 1 (guardado sin conexión 2).ntb";
        check(p && leer(destino) == "cambiado en la red mientras tanto" && leer(al_lado2) == "version 4 sin red",
              "si el de la red cambio desde la base tampoco se reemplaza (destino: " + leer(destino) + ", al lado: " + leer(al_lado2) +
                  ", escrito: " + u(escrito(p, destino)) + ")");
        sello = sello_de(destino);
        DeleteFileW(al_lado.c_str());
        DeleteFileW(al_lado2.c_str());
    }

    std::puts("== trabajos: nombres y carpetas que no se aceptan");
    check(!guardar_trabajo(N + L"\\perfiles.ntb", "x", sello_nuevo(), en_espera, base, error) &&
              !guardar_trabajo(L"C:\\nt\\PERFILES.NTB", "x", sello_nuevo(), en_espera, base, error) && !existe(N + L"\\perfiles.ntb"),
          "perfiles.ntb es del catalogo: no se guarda un trabajo con ese nombre");
    check(!guardar_trabajo(carpeta_espera() + L"\\suelto.ntb", "x", sello_nuevo(), en_espera, base, error) &&
              !existe(carpeta_espera() + L"\\suelto.ntb"),
          "dentro de la carpeta de espera no se guarda");
    crear_carpetas(carpeta_espera());
    escribir(carpeta_espera() + L"\\suelto.ntb", "sin destino");
    p = pasada(destino);
    check(p && p->trabajos_en_espera == 0 && p->esperando_red.empty() && existe(carpeta_espera() + L"\\suelto.ntb"),
          "un archivo sin destino en la espera no cuenta como trabajo esperando");
    DeleteFileW((carpeta_espera() + L"\\suelto.ntb").c_str());

    std::puts("== trabajos: letra que vuelve como disco local");
    {
        const std::wstring local = L"C:\\nt_pruebas\\Obra.ntb";
        borrar_arbol(L"C:\\nt_pruebas");
        crear_carpetas(L"C:\\nt_pruebas");
        escribir(local, "v1 en el disco");
        // como si se hubiera guardado cuando la letra no existia
        const std::wstring pl = ruta_en_espera(local);
        crear_carpetas(carpeta_de(pl));
        escribir(pl, "v2 guardada sin la letra");
        a_leer = version_a_abrir(local, d, en_espera, base);
        check(a_leer == pl && en_espera && d == local, "Abrir lee la version en espera aunque la letra ya no sea de red");
        Sello s;
        check(guardar_trabajo(local, "v3 directo", base, en_espera, s, error) && !en_espera && leer(local) == "v3 directo" && !existe(pl),
              "guardar directo borra la version en espera (es mas vieja)");
        p = pasada(local);
        check(p && leer(local) == "v3 directo", "y la pasada ya no la copia encima");
        borrar_arbol(L"C:\\nt_pruebas");
    }

    std::puts("== catalogo");
    std::vector<PerfilTxt> cat;
    check(cargar_catalogo(cat, error) && cat.empty(), "sin catalogo: vacio y sin error");
    CambiosCatalogo c;
    c.cambiados = {P("Tubo 40x40", "6400"), P("Platina 50x5", "6000")};
    check(registrar(c, cat, error) && nombres(cat) == "Tubo 40x40=6400,Platina 50x5=6000" &&
              !existe(carpeta_local() + L"\\perfiles_pendientes.ntb"),
          "sin carpeta de datos: solo la copia local, sin pendientes");
    // elegir N:\datos (no tiene catalogo)
    ultimo_examen.reset();
    examinar(N);
    check(esperar(true, 20000) && ultimo_examen->lectura == Lectura::no_existe, "examinar N:\\datos: no tiene catalogo");
    check(cambiar_carpeta(N, Lectura::no_existe, {}, true, cat, error) && leer_config(L"carpeta_datos") == N &&
              leer_config(L"carpeta") == N && leer_config(L"carpeta_exportar") == N,
          "cambiar de carpeta: config apunta a N:\\datos");
    p = pasada();
    check(p && nombres(catalogo_en(N + L"\\perfiles.ntb")) == "Tubo 40x40=6400,Platina 50x5=6000" && !p->catalogo_en_espera &&
              !existe(carpeta_local() + L"\\perfiles_pendientes.ntb") && p->hay_catalogo,
          "la pasada sube el catalogo y borra los pendientes");
    // editar sin red: queda pendiente y llega al volver
    MoveFileW(srv.c_str(), srv_off.c_str());
    CambiosCatalogo e1;
    e1.cambiados = {P("Tubo 40x40", "6500")};
    check(registrar(e1, cat, error) && existe(carpeta_local() + L"\\perfiles_pendientes.ntb"), "editar sin red: cambio pendiente");
    p = pasada();
    check(p && p->catalogo_en_espera && conexion(p, L"N:\\") == Conexion::sin_conexion, "sin red: sigue pendiente");
    MoveFileW(srv_off.c_str(), srv.c_str());
    p = pasada();
    check(p && nombres(catalogo_en(N + L"\\perfiles.ntb")) == "Tubo 40x40=6500,Platina 50x5=6000" && !p->catalogo_en_espera,
          "al volver la red el cambio llega a la carpeta");
    // corte entre las dos escrituras: pendientes con el cambio, copia local sin el
    {
        CambiosCatalogo e2;
        e2.cambiados = {P("Angulo 30", "6000")};
        escribir(carpeta_local() + L"\\perfiles_pendientes.ntb", a_texto_cambios(e2));
        std::vector<PerfilTxt> al_arrancar;
        cargar_catalogo(al_arrancar, error);
        check(nombres(al_arrancar) == "Tubo 40x40=6500,Platina 50x5=6000,Angulo 30=6000", "corte entre escrituras: al arrancar se recupera");
        pasada();
        check(nombres(catalogo_en(N + L"\\perfiles.ntb")) == "Tubo 40x40=6500,Platina 50x5=6000,Angulo 30=6000", "y llega a la carpeta");
    }
    // catalogo de la carpeta ilegible
    escribir(N + L"\\perfiles.ntb", "hola");
    CambiosCatalogo e3;
    e3.cambiados = {P("Rect 80x40", "6000")};
    registrar(e3, cat, error);
    p = pasada();
    check(p && !p->error_catalogo.empty() && leer(N + L"\\perfiles.ntb") == "hola" && p->catalogo_en_espera,
          "catalogo ilegible en la carpeta: no se toca y se informa (" + u(p ? p->error_catalogo : L"") + ")");
    check(nombres(copia_local({})) == "Tubo 40x40=6500,Platina 50x5=6000,Angulo 30=6000,Rect 80x40=6000",
          "la copia local conserva todo");
    // dos ventanas con el catalogo en memoria distinto editan campos distintos del mismo perfil
    {
        std::vector<PerfilTxt> a = copia_local({}), b = a;
        PerfilTxt fb = b[0];
        fb.barra = "6600";
        check(registrar([&](const std::vector<PerfilTxt>& x) { return cambio_por_campo(x, fb, 2); }, b, error), "ventana B cambia la barra");
        PerfilTxt fa = a[0];   // A no se entero
        fa.despunte = "15";
        check(registrar([&](const std::vector<PerfilTxt>& x) { return cambio_por_campo(x, fa, 3); }, a, error), "ventana A cambia el despunte");
        auto l = copia_local({});
        check(l[0].barra == "6600" && l[0].despunte == "15" && a[0].barra == "6600",
              "quedan las dos ediciones: el cambio se calcula sobre la copia en disco, no sobre la memoria");
        CambiosCatalogo vuelta;
        PerfilTxt f0 = l[0];
        f0.barra = "6500";
        f0.despunte = "10";
        vuelta.cambiados = {f0};
        registrar(vuelta, a, error);
    }
    // copia local ilegible: no se reemplaza por lo que hay en memoria
    {
        std::string antes = leer(carpeta_local() + L"\\perfiles.ntb");
        escribir(carpeta_local() + L"\\perfiles.ntb", "basura");
        std::vector<PerfilTxt> mem = {P("Solo en memoria", "6000")};
        CambiosCatalogo x;
        x.cambiados = {P("Uno nuevo", "6000")};
        check(!registrar(x, mem, error) && leer(carpeta_local() + L"\\perfiles.ntb") == "basura" && !error.empty(),
              "copia local ilegible: registrar no escribe nada y lo dice");
        escribir(carpeta_local() + L"\\perfiles.ntb", antes);
    }
    // no se pueden anotar los pendientes: tampoco se toca la copia local
    {
        std::string antes = leer(carpeta_local() + L"\\perfiles.ntb"), pend = leer(carpeta_local() + L"\\perfiles_pendientes.ntb");
        DeleteFileW((carpeta_local() + L"\\perfiles_pendientes.ntb").c_str());
        CreateDirectoryW((carpeta_local() + L"\\perfiles_pendientes.ntb").c_str(), nullptr);
        std::vector<PerfilTxt> mem = copia_local({});
        CambiosCatalogo x;
        x.cambiados = {P("Uno nuevo", "6000")};
        check(!registrar(x, mem, error) && leer(carpeta_local() + L"\\perfiles.ntb") == antes && buscar_perfil(mem, "Uno nuevo") < 0,
              "sin poder anotar los pendientes: la copia local no cambia (si no, la pasada borraria la edicion)");
        RemoveDirectoryW((carpeta_local() + L"\\perfiles_pendientes.ntb").c_str());
        if (!pend.empty()) escribir(carpeta_local() + L"\\perfiles_pendientes.ntb", pend);
    }
    DeleteFileW((N + L"\\perfiles.ntb").c_str());
    CreateDirectoryW((N + L"\\perfiles.ntb").c_str(), nullptr);
    p = pasada();
    check(p && !p->error_catalogo.empty(), "una carpeta llamada perfiles.ntb: error, no vacio");
    RemoveDirectoryW((N + L"\\perfiles.ntb").c_str());
    p = pasada();
    check(p && p->error_catalogo.empty() &&
              nombres(catalogo_en(N + L"\\perfiles.ntb")) == "Tubo 40x40=6500,Platina 50x5=6000,Angulo 30=6000,Rect 80x40=6000",
          "si se borra, se vuelve a crear completo");
    // otra carpeta con catalogo distinto
    CreateDirectoryW((srv + L"\\otra").c_str(), nullptr);
    {
        Trabajo t;
        t.perfiles = {P("Tubo 40x40", "6000"), P("Solo alla", "6000")};
        escribir(L"N:\\otra\\perfiles.ntb", a_texto(t));
    }
    ultimo_examen.reset();
    examinar(L"N:\\otra");
    check(esperar(true, 20000) && ultimo_examen->lectura == Lectura::ok && nombres(ultimo_examen->catalogo) == "Tubo 40x40=6000,Solo alla=6000",
          "examinar otra carpeta con catalogo");
    auto local = copia_local({});
    check(perfiles_distintos(ultimo_examen->catalogo, local) == std::vector<std::string>{"Tubo 40x40"}, "un perfil distinto");
    // mientras la pregunta esta abierta, otra ventana agrega un perfil
    CambiosCatalogo en_b;
    en_b.cambiados = {P("Nuevo en B", "6000")};
    std::vector<PerfilTxt> mem_b = local;
    registrar(en_b, mem_b, error);
    std::vector<PerfilTxt> unido;
    check(cambiar_carpeta(L"N:\\otra", ultimo_examen->lectura, ultimo_examen->catalogo, true, unido, error) &&
              existe(carpeta_local() + L"\\perfiles antes de cambiar de carpeta.ntb"),
          "cambiar a la otra carpeta deja un respaldo");
    check(buscar_perfil(unido, "Nuevo en B") >= 0 && buscar_perfil(copia_local({}), "Nuevo en B") >= 0,
          "la mezcla usa la copia local de ese momento: no se pierde lo que otra ventana agrego mientras tanto");
    p = pasada();
    check(nombres(catalogo_en(L"N:\\otra\\perfiles.ntb")) ==
              "Tubo 40x40=6000,Solo alla=6000,Platina 50x5=6000,Angulo 30=6000,Rect 80x40=6000,Nuevo en B=6000",
          "la otra carpeta queda con lo suyo mas lo de este equipo");
    ultimo_examen.reset();
    examinar(L"N:\\no existe");
    check(esperar(true, 20000) && ultimo_examen->lectura == Lectura::error, "examinar una carpeta que no existe: error");

    std::puts("== esperar al cerrar");
    guardar_trabajo(destino, "version 5", sello, en_espera, sello, error);
    unsigned long long n = pedir_pasada(destino);
    check(esperar_pasada(n, 5000) && leer(destino) == "version 5", "esperar_pasada vuelve cuando la pasada termino");

    std::puts("== migracion desde 0.2");
    borrar_arbol(carpeta_local());
    DeleteFileW((carpeta_config() + L"\\config.ini").c_str());
    {
        Trabajo t;
        t.perfiles = {P("De la 0.2", "6000")};
        escribir(carpeta_config() + L"\\perfiles.ntb", a_texto(t));
    }
    CreateDirectoryW(carpeta_local().c_str(), nullptr);
    check(cargar_catalogo(cat, error) && nombres(cat) == "De la 0.2=6000" && existe(carpeta_local() + L"\\perfiles.ntb"),
          "la copia de %APPDATA% pasa a %LOCALAPPDATA%");

    std::printf("\nFALLOS: %d\n", fallos);
    return fallos ? 1 : 0;
}
