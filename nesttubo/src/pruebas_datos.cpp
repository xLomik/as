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

static InformePasada* pasada(const std::wstring& archivo = L"") {
    ultima_pasada.reset();
    pedir_pasada(archivo);
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
    bool guardado = guardar_trabajo(destino, "version 1", en_espera, error);
    check(guardado && en_espera && leer(pe) == "version 1" && !existe(destino), "Guardar en red: primero queda en este equipo " + u(error));
    std::wstring a_leer = version_a_abrir(destino, d, en_espera);
    check(a_leer == pe && d == destino && en_espera, "Abrir el destino lee la version en espera");
    a_leer = version_a_abrir(pe, d, en_espera);
    check(a_leer == pe && d == destino && en_espera, "Abrir el archivo en espera: el destino es el de red");
    InformePasada* p = pasada(destino);
    check(p && leer(destino) == "version 1" && !existe(pe) && p->copiados.size() == 1 && p->trabajos_en_espera == 0,
          "la pasada lo copia y borra el de este equipo");
    check(!existe(carpeta_espera() + L"\\N\\datos\\Cliente A") && !existe(carpeta_espera() + L"\\N"),
          "y borra las carpetas vacias de la espera");
    check(conexion(p, L"N:\\") == Conexion::en_linea, "N: en linea");
    a_leer = version_a_abrir(destino, d, en_espera);
    check(a_leer == destino && !en_espera, "sin version en espera, Abrir lee el destino");

    std::puts("== trabajos: sin red");
    check(MoveFileW(srv.c_str(), srv_off.c_str()), "desconectar la red (mover la carpeta del servidor)");
    check(guardar_trabajo(destino, "version 2", en_espera, error) && en_espera, "Guardar sin red funciona igual");
    const std::wstring otro = N + L"\\Cliente B\\Pedido 1.ntb";
    check(guardar_trabajo(otro, "cliente B", en_espera, error) && en_espera && ruta_en_espera(otro) != pe,
          "dos 'Pedido 1.ntb' de carpetas distintas: dos archivos en espera");
    p = pasada(destino);
    check(p && conexion(p, L"N:\\") == Conexion::sin_conexion && p->trabajos_en_espera == 2 && p->fallas.empty() && p->copiados.empty(),
          "sin red: N: sin conexion, 2 en espera y ningun error real");
    check(MoveFileW(srv_off.c_str(), srv.c_str()), "reconectar");
    p = pasada(destino);
    check(p && leer(destino) == "version 2" && leer(otro) == "cliente B" && p->trabajos_en_espera == 0 && !existe(carpeta_espera() + L"\\N"),
          "al volver la red se copian los dos, cada uno a su carpeta");

    std::puts("== trabajos: guardar otra vez con red");
    check(guardar_trabajo(destino, "version 3", en_espera, error), "guardar version 3");
    p = pasada(destino);
    check(p && leer(destino) == "version 3", "version 3 copiada encima de la 2");

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
    check(cambiar_carpeta(N, copia_local(cat), false, error) && leer_config(L"carpeta_datos") == N &&
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
    auto unido = unir_catalogos(ultimo_examen->catalogo, local);
    check(cambiar_carpeta(L"N:\\otra", unido, true, error) && existe(carpeta_local() + L"\\perfiles antes de cambiar de carpeta.ntb"),
          "cambiar a la otra carpeta deja un respaldo");
    p = pasada();
    check(nombres(catalogo_en(L"N:\\otra\\perfiles.ntb")) == "Tubo 40x40=6000,Solo alla=6000,Platina 50x5=6000,Angulo 30=6000,Rect 80x40=6000",
          "la otra carpeta queda con lo suyo mas lo de este equipo");
    ultimo_examen.reset();
    examinar(L"N:\\no existe");
    check(esperar(true, 20000) && ultimo_examen->lectura == Lectura::error, "examinar una carpeta que no existe: error");

    std::puts("== esperar al cerrar");
    guardar_trabajo(destino, "version 4", en_espera, error);
    unsigned long long n = pedir_pasada(destino);
    check(esperar_pasada(n, 5000) && leer(destino) == "version 4", "esperar_pasada vuelve cuando la pasada termino");

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
