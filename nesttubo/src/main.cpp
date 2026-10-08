// NestTubo - ventana Win32. Solo muestra y edita: el calculo vive en nucleo.cpp,
// la lectura del trabajo en trabajo.cpp y los archivos y la red en datos.cpp.
#include <windows.h>
#include <commctrl.h>
#include <commdlg.h>
#include <shellapi.h>
#include <shlobj.h>
#include <shobjidl.h>

#include <algorithm>
#include <atomic>
#include <chrono>
#include <map>
#include <memory>
#include <set>
#include <string>
#include <thread>
#include <vector>

#include "datos.h"
#include "nucleo.h"
#include "salidas.h"
#include "trabajo.h"

using namespace nt;
using datos::Conexion;
using datos::U;
using datos::W;

namespace {

// Tope de tiempo por perfil: al vencer se entrega lo mejor que haya.
constexpr int TOPE_SEGUNDOS = 20;

enum : int {
    ID_PERFILES = 101, ID_PIEZAS, ID_RESUMEN, ID_PLAN, ID_ESTADO,
    ID_NUEVO = 201, ID_ABRIR, ID_GUARDAR, ID_GUARDAR_COMO, ID_CALCULAR, ID_CANCELAR, ID_QUITAR_PIEZA, ID_QUITAR_PERFIL, ID_EXPORTAR,
    ID_CARPETA_DATOS,
    ID_ET_PERFILES = 301, ID_ET_PIEZAS, ID_ET_RESUMEN, ID_ET_PLAN,
};
enum : UINT { WM_APP_MOVER = WM_APP + 1, WM_APP_PROGRESO, WM_APP_FIN, WM_APP_PENDIENTES };
enum Movimiento { MOV_SIGUIENTE_COL = 1, MOV_ANTERIOR_COL, MOV_ABAJO, MOV_ARRIBA };

// ---------------------------------------------------------------------------
// Estado de la ventana

struct Resultado {
    std::vector<ResultadoPerfil> perfiles;
};

struct App {
    HINSTANCE inst = nullptr;
    HWND wnd = nullptr, perfiles = nullptr, piezas = nullptr, resumen = nullptr, plan = nullptr, estado = nullptr;
    HFONT fuente = nullptr, negrita = nullptr;
    int dpi = 96;
    Trabajo trabajo;
    std::wstring archivo;
    datos::Sello sello;   // como estaba `archivo` la ultima vez que esta ventana lo leyo o lo escribio
    bool modificado = false;
    // edicion de una celda
    HWND edit = nullptr, edit_lv = nullptr;
    int edit_fila = -1, edit_col = -1;
    bool cerrando_edit = false;
    // calculo
    std::thread hilo;
    std::atomic<bool> cancelar{false};
    bool calculando = false;
    bool resultado_viejo = false;
    std::vector<ResultadoPerfil> resultados;
    std::vector<char> plan_sombra;   // filas del plan con fondo gris: un grupo de barras si, otro no
    // catalogo de perfiles y carpeta de datos
    std::vector<PerfilTxt> catalogo;              // aparte de la tabla: la tabla es la del trabajo abierto
    std::map<std::wstring, Conexion> conexion;    // datos::clave_ruta(raiz) -> lo ultimo que informo el hilo
    int trabajos_en_espera = 0, fallas = 0;
    std::vector<std::pair<std::wstring, int>> esperando_red;   // raiz sin conexion -> trabajos que la esperan
    bool catalogo_en_espera = false;
    std::wstring error_catalogo;
    std::set<std::wstring> fallas_avisadas;       // trabajo en espera + codigo: un solo cuadro por cada uno
    bool aviso_sin_conexion = false;              // el cuadro de Guardar sin conexion sale una vez por sesion
    bool perfiles_sin_anotar = false;             // un cambio de perfiles no quedo en el catalogo: no perderlo al cerrar
    bool examinando = false;
    // Lo que llega del hilo y abre un cuadro espera a que no haya una celda en
    // edicion ni otro cuadro abierto: el cuadro le quitaria el foco a la celda y
    // confirmaria lo escrito a medias, o cambiaria la tabla debajo de una pregunta.
    int modal = 0;                                // cuadros y dialogos abiertos
    std::vector<std::wstring> avisos;
    std::unique_ptr<datos::InformeExaminar> examen;
    std::wstring texto_datos;                     // estado de la carpeta de datos (barra de estado)
    bool datos_rojo = false;
} app;

int S(int v) { return MulDiv(v, app.dpi, 96); }

bool ocupado() { return app.edit || app.modal > 0; }

void revisar_pendientes() {
    if (app.examen || !app.avisos.empty()) PostMessageW(app.wnd, WM_APP_PENDIENTES, 0, 0);
}

int cuadro(const std::wstring& texto, UINT tipo) {
    app.modal++;
    int r = MessageBoxW(app.wnd, texto.c_str(), L"NestTubo", tipo);
    app.modal--;
    revisar_pendientes();
    return r;
}

// Columnas de las dos tablas de entrada
const wchar_t* COLS_PERFILES[] = {L"Perfil", L"Cara ancha", L"Barra", L"Despunte", L"Zona muerta", L"Separación",
                                   L"Margen"};
const int ANCHO_PERFILES[] = {165, 80, 65, 75, 90, 85, 65};
const wchar_t* COLS_PIEZAS[] = {L"Perfil", L"Pieza", L"Largo (mm)", L"Ángulo 1", L"Ángulo 2", L"Cantidad"};
const int ANCHO_PIEZAS[] = {165, 140, 90, 75, 75, 75};

bool es_perfiles(HWND lv) { return lv == app.perfiles; }
int columnas(HWND lv) { return es_perfiles(lv) ? 7 : 6; }

std::string* celda(HWND lv, int fila, int col) {
    if (es_perfiles(lv)) {
        if (fila < 0 || fila >= (int)app.trabajo.perfiles.size()) return nullptr;
        PerfilTxt& p = app.trabajo.perfiles[fila];
        std::string* c[] = {&p.nombre, &p.cara, &p.barra, &p.despunte, &p.zona_muerta, &p.separacion, &p.margen};
        return c[col];
    }
    if (fila < 0 || fila >= (int)app.trabajo.piezas.size()) return nullptr;
    PiezaTxt& p = app.trabajo.piezas[fila];
    std::string* c[] = {&p.perfil, &p.nombre, &p.largo, &p.angulo1, &p.angulo2, &p.cantidad};
    return c[col];
}

int filas(HWND lv) { return es_perfiles(lv) ? (int)app.trabajo.perfiles.size() : (int)app.trabajo.piezas.size(); }

void estado(const std::wstring& t) { SendMessageW(app.estado, SB_SETTEXTW, 0, (LPARAM)t.c_str()); }

Conexion conexion_de(const std::wstring& ruta) {
    auto it = app.conexion.find(datos::clave_ruta(datos::raiz(ruta)));
    return it == app.conexion.end() ? Conexion::desconocida : it->second;
}

// Ruta de red cuya raiz no se sabe en linea: la ventana no la toca (podria colgarse).
bool fuera_de_linea(const std::wstring& ruta) { return datos::es_remota(ruta) && conexion_de(ruta) != Conexion::en_linea; }

std::wstring cuantos(int n, const wchar_t* uno, const wchar_t* varios) { return std::to_wstring(n) + L" " + (n == 1 ? uno : varios); }

// Estado de la carpeta de datos, en la parte derecha de la barra de estado (en rojo si algo espera o falla).
void etiqueta_datos() {
    std::wstring c = datos::leer_config(L"carpeta_datos");
    int sin_red = 0;
    for (const auto& x : app.esperando_red) sin_red += x.second;
    int copiando = std::max(0, app.trabajos_en_espera - sin_red);
    std::wstring t, espera, en_curso;
    bool rojo = false;
    bool cambios = app.catalogo_en_espera && !c.empty();
    if (app.trabajos_en_espera > 0) espera = cuantos(app.trabajos_en_espera, L"trabajo", L"trabajos");
    if (cambios) espera += (espera.empty() ? L"" : L" y ") + std::wstring(L"cambios de perfiles");
    if (copiando > 0) en_curso = cuantos(copiando, L"trabajo", L"trabajos");
    if (cambios) en_curso += (en_curso.empty() ? L"" : L" y ") + std::wstring(L"cambios de perfiles");
    Conexion e = conexion_de(c);
    if (app.examinando) {
        t = L"Revisando la carpeta elegida...";
    } else if (!c.empty() && e == Conexion::desconocida) {
        t = L"Comprobando " + c + L"...";
    } else if (!c.empty() && e == Conexion::sin_conexion) {
        rojo = true;
        t = L"Sin conexión con " + c + (espera.empty() ? L"" : L": " + espera + L" esperando en este equipo");
    } else if (!app.error_catalogo.empty()) {
        rojo = true;
        t = app.error_catalogo;
    } else if (app.fallas > 0) {
        rojo = true;
        t = L"Error al copiar: " + cuantos(app.fallas, L"trabajo sin copiar", L"trabajos sin copiar");
    } else if (sin_red > 0) {
        rojo = true;
        t = cuantos(sin_red, L"trabajo esperando", L"trabajos esperando") + L" conexión con " +
            (app.esperando_red.size() == 1 ? app.esperando_red[0].first : L"varias carpetas de red") +
            (sin_red == 1 ? L" (guardado en este equipo)" : L" (guardados en este equipo)");
    } else {
        t = c.empty() ? L"Carpeta de datos: este equipo" : L"Carpeta de datos: " + c;
        if (!en_curso.empty()) t += L" (copiando " + en_curso + L")";
    }
    if (t == app.texto_datos && rojo == app.datos_rojo) return;
    app.texto_datos = t;
    app.datos_rojo = rojo;
    SendMessageW(app.estado, SB_SETTEXTW, 1 | SBT_OWNERDRAW, (LPARAM)app.texto_datos.c_str());
    InvalidateRect(app.estado, nullptr, TRUE);
}

// Unico camino para cambiar el catalogo desde la ventana: lo anota en disco al
// momento. El cambio se calcula sobre la copia en disco, no sobre app.catalogo.
bool anotar(const datos::Cambio& cambio) {
    std::wstring error;
    bool ok = datos::registrar(cambio, app.catalogo, error);
    if (!error.empty()) cuadro(error, ok ? MB_ICONWARNING : MB_ICONERROR);
    if (!datos::leer_config(L"carpeta_datos").empty()) {
        app.catalogo_en_espera = true;
        datos::pedir_pasada(app.archivo);
    }
    if (ok) app.perfiles_sin_anotar = false;
    else app.perfiles_sin_anotar = true;   // quedo solo en la tabla del trabajo: guardar el trabajo para no perderlo
    etiqueta_datos();
    return ok;
}

void titulo() {
    std::wstring t = L"NestTubo - ";
    if (app.archivo.empty()) t += L"trabajo nuevo";
    else {
        size_t k = app.archivo.find_last_of(L"\\/");
        t += k == std::wstring::npos ? app.archivo : app.archivo.substr(k + 1);
    }
    if (app.modificado) t += L" *";
    SetWindowTextW(app.wnd, t.c_str());
}

// La ultima fila de cada tabla de entrada siempre esta vacia: ahi se escribe la siguiente.
void asegurar_fila_vacia() {
    if (app.trabajo.perfiles.empty() || !fila_vacia(app.trabajo.perfiles.back())) app.trabajo.perfiles.push_back(PerfilTxt{});
    if (app.trabajo.piezas.empty() || !fila_vacia(app.trabajo.piezas.back())) app.trabajo.piezas.push_back(PiezaTxt{});
}

void poner_texto(HWND lv, int fila, int col, const std::wstring& t) {
    ListView_SetItemText(lv, fila, col, const_cast<wchar_t*>(t.c_str()));
}

void llenar_tabla(HWND lv) {
    SendMessageW(lv, WM_SETREDRAW, FALSE, 0);
    ListView_DeleteAllItems(lv);
    int n = filas(lv), nc = columnas(lv);
    for (int f = 0; f < n; f++) {
        LVITEMW it{};
        it.mask = LVIF_TEXT;
        it.iItem = f;
        std::wstring t0 = W(*celda(lv, f, 0));
        it.pszText = const_cast<wchar_t*>(t0.c_str());
        ListView_InsertItem(lv, &it);
        for (int c = 1; c < nc; c++) poner_texto(lv, f, c, W(*celda(lv, f, c)));
    }
    SendMessageW(lv, WM_SETREDRAW, TRUE, 0);
    InvalidateRect(lv, nullptr, TRUE);
}

void agregar_fila_lv(HWND lv) {   // refleja en la tabla la fila que se acaba de anadir al modelo
    int f = filas(lv) - 1;
    LVITEMW it{};
    it.mask = LVIF_TEXT;
    it.iItem = f;
    std::wstring t0 = W(*celda(lv, f, 0));
    it.pszText = const_cast<wchar_t*>(t0.c_str());
    ListView_InsertItem(lv, &it);
    for (int c = 1; c < columnas(lv); c++) poner_texto(lv, f, c, W(*celda(lv, f, c)));
}

void datos_cambiaron() {
    app.modificado = true;
    titulo();
    if (!app.resultados.empty() && !app.resultado_viejo) {
        app.resultado_viejo = true;
        estado(L"Los datos cambiaron desde el último cálculo: vuelve a calcular.");
    }
}

// ---------------------------------------------------------------------------
// Edicion de celdas: un EDIT superpuesto a la celda (ListView no edita subcolumnas).

LRESULT CALLBACK EditProc(HWND h, UINT m, WPARAM w, LPARAM l, UINT_PTR, DWORD_PTR) {
    switch (m) {
    case WM_GETDLGCODE:
        return DLGC_WANTALLKEYS;
    case WM_KEYDOWN:
        switch (w) {
        case VK_TAB:
            PostMessageW(app.wnd, WM_APP_MOVER, (GetKeyState(VK_SHIFT) < 0) ? MOV_ANTERIOR_COL : MOV_SIGUIENTE_COL, 0);
            return 0;
        case VK_RETURN:
        case VK_DOWN:
            PostMessageW(app.wnd, WM_APP_MOVER, MOV_ABAJO, 0);
            return 0;
        case VK_UP:
            PostMessageW(app.wnd, WM_APP_MOVER, MOV_ARRIBA, 0);
            return 0;
        case VK_ESCAPE:
            PostMessageW(app.wnd, WM_APP_MOVER, 0, 1);   // cancelar
            return 0;
        }
        break;
    case WM_CHAR:
        if (w == L'\t' || w == L'\r' || w == 27) return 0;   // sin pitido
        break;
    case WM_KILLFOCUS:
        PostMessageW(app.wnd, WM_APP_MOVER, 0, 0);   // confirmar y cerrar
        break;
    }
    return DefSubclassProc(h, m, w, l);
}

void perfil_si_falta(const std::string& nombre);

int fila_con_perfil(const std::string& nombre, int salvo) {
    std::string k = clave_perfil(nombre);
    for (int i = 0; i < (int)app.trabajo.perfiles.size(); i++)
        if (i != salvo && !k.empty() && clave_perfil(app.trabajo.perfiles[i].nombre) == k) return i;
    return -1;
}

// Edicion a mano de la celda (f, c) de la tabla de perfiles: cambia la fila y
// anota en el catalogo solo lo que se edito. false si no se acepta.
bool editar_perfil(int f, int c, const std::string& nuevo) {
    PerfilTxt& p = app.trabajo.perfiles[f];
    std::string viejo = *campo_perfil(p, c);
    if (c == 0) {
        int otra = fila_con_perfil(nuevo, f);
        if (otra >= 0) {   // dos filas con el mismo perfil: quitar una borraria el perfil del catalogo
            MessageBeep(MB_ICONWARNING);
            estado(L"Ya hay un perfil \"" + W(app.trabajo.perfiles[otra].nombre) + L"\" en la fila " + std::to_wstring(otra + 1) +
                   L". No se cambió el nombre.");
            return false;
        }
    }
    *campo_perfil(p, c) = nuevo;
    if (c == 0) {
        app.catalogo = datos::catalogo_vigente(app.catalogo);   // otra ventana pudo agregarlo
        int ic = buscar_perfil(app.catalogo, nuevo);
        if (clave_perfil(viejo).empty() && ic >= 0) {
            p = app.catalogo[ic];
            estado(L"Perfil \"" + W(p.nombre) + L"\" traído del catálogo.");
        } else {
            if (clave_perfil(viejo).empty()) {   // perfil nuevo: valores iniciales en lo que no se haya escrito
                PerfilTxt ini = perfil_nuevo(p.nombre);
                for (int k = 2; k < 7; k++)
                    if (campo_perfil(p, k)->empty()) *campo_perfil(p, k) = *campo_perfil(ini, k);
            }
            PerfilTxt fila = p;
            bool sigue = fila_con_perfil(viejo, f) >= 0;
            anotar([&](const std::vector<PerfilTxt>& actual) { return cambio_por_nombre(actual, fila, viejo, sigue); });
        }
    } else if (!clave_perfil(p.nombre).empty()) {
        PerfilTxt fila = p;
        anotar([&](const std::vector<PerfilTxt>& actual) { return cambio_por_campo(actual, fila, c); });
    }
    for (int k = 0; k < 7; k++) poner_texto(app.perfiles, f, k, W(*celda(app.perfiles, f, k)));
    return true;
}

// Guarda el texto del EDIT en el modelo y cierra el EDIT. cancelar = descartar lo escrito.
void cerrar_edicion(bool cancelar) {
    if (!app.edit || app.cerrando_edit) return;
    app.cerrando_edit = true;
    HWND lv = app.edit_lv;
    int f = app.edit_fila, c = app.edit_col;
    if (!cancelar) {
        int n = GetWindowTextLengthW(app.edit);
        std::wstring t(n, L'\0');
        if (n) GetWindowTextW(app.edit, &t[0], n + 1);
        std::string nuevo = U(t);
        std::string* cel = celda(lv, f, c);
        if (cel && *cel != nuevo && (!es_perfiles(lv) || editar_perfil(f, c, nuevo))) {
            if (!es_perfiles(lv)) {
                bool era_vacia = fila_vacia(app.trabajo.piezas[f]);
                *cel = nuevo;
                poner_texto(lv, f, c, t);
                if (era_vacia && !nuevo.empty() && c != 0 && f > 0 && app.trabajo.piezas[f].perfil.empty()) {
                    // pieza nueva: mismo perfil que la fila de arriba
                    app.trabajo.piezas[f].perfil = app.trabajo.piezas[f - 1].perfil;
                    poner_texto(lv, f, 0, W(app.trabajo.piezas[f].perfil));
                }
                if (c == 0) perfil_si_falta(nuevo);
            }
            size_t antes_perf = app.trabajo.perfiles.size(), antes_piez = app.trabajo.piezas.size();
            asegurar_fila_vacia();
            if (app.trabajo.perfiles.size() > antes_perf) agregar_fila_lv(app.perfiles);
            if (app.trabajo.piezas.size() > antes_piez) agregar_fila_lv(app.piezas);
            datos_cambiaron();
        }
    }
    HWND e = app.edit;
    app.edit = nullptr;
    DestroyWindow(e);
    app.cerrando_edit = false;
    revisar_pendientes();
}

void iniciar_edicion(HWND lv, int f, int c) {
    cerrar_edicion(false);
    if (app.calculando || f < 0 || f >= filas(lv) || c < 0 || c >= columnas(lv)) return;
    ListView_EnsureVisible(lv, f, FALSE);
    RECT r;
    if (c == 0) ListView_GetSubItemRect(lv, f, 0, LVIR_LABEL, &r);
    else ListView_GetSubItemRect(lv, f, c, LVIR_BOUNDS, &r);
    // que la columna entera quede a la vista
    RECT cli;
    GetClientRect(lv, &cli);
    if (r.right > cli.right) ListView_Scroll(lv, r.right - cli.right, 0);
    else if (r.left < 0) ListView_Scroll(lv, r.left, 0);
    if (c == 0) ListView_GetSubItemRect(lv, f, 0, LVIR_LABEL, &r);
    else ListView_GetSubItemRect(lv, f, c, LVIR_BOUNDS, &r);
    ListView_SetItemState(lv, -1, 0, LVIS_SELECTED);
    ListView_SetItemState(lv, f, LVIS_SELECTED | LVIS_FOCUSED, LVIS_SELECTED | LVIS_FOCUSED);
    app.edit = CreateWindowExW(0, L"EDIT", W(*celda(lv, f, c)).c_str(), WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL,
                               r.left, r.top, r.right - r.left, r.bottom - r.top + 1, lv, nullptr, app.inst, nullptr);
    SendMessageW(app.edit, WM_SETFONT, (WPARAM)app.fuente, TRUE);
    SetWindowSubclass(app.edit, EditProc, 1, 0);
    app.edit_lv = lv;
    app.edit_fila = f;
    app.edit_col = c;
    SetFocus(app.edit);
    SendMessageW(app.edit, EM_SETSEL, 0, -1);
}

void mover_edicion(int mov) {
    if (!app.edit) return;
    HWND lv = app.edit_lv;
    int f = app.edit_fila, c = app.edit_col, nc = columnas(lv);
    cerrar_edicion(false);
    switch (mov) {
    case MOV_SIGUIENTE_COL: if (++c >= nc) { c = 0; f++; } break;
    case MOV_ANTERIOR_COL: if (--c < 0) { c = nc - 1; f--; } break;
    case MOV_ABAJO: f++; break;
    case MOV_ARRIBA: f--; break;
    }
    if (f >= 0 && f < filas(lv)) iniciar_edicion(lv, f, c);
    else SetFocus(lv);
}

// Si la pieza nombra un perfil que no esta en la tabla, se trae del catalogo; si
// tampoco esta ahi, se agrega con los valores iniciales y se anota en el catalogo.
void perfil_si_falta(const std::string& nombre) {
    if (clave_perfil(nombre).empty() || buscar_perfil(app.trabajo.perfiles, nombre) >= 0) return;
    app.catalogo = datos::catalogo_vigente(app.catalogo);   // otra ventana pudo agregarlo
    int ic = buscar_perfil(app.catalogo, nombre);
    PerfilTxt p = ic >= 0 ? app.catalogo[ic] : perfil_nuevo(nombre);
    if (!app.trabajo.perfiles.empty() && fila_vacia(app.trabajo.perfiles.back())) {
        app.trabajo.perfiles.back() = p;
        int f = (int)app.trabajo.perfiles.size() - 1;
        for (int k = 0; k < 7; k++) poner_texto(app.perfiles, f, k, W(*celda(app.perfiles, f, k)));
    } else {
        app.trabajo.perfiles.push_back(p);
        agregar_fila_lv(app.perfiles);
    }
    if (ic >= 0) {
        estado(L"Perfil \"" + W(p.nombre) + L"\" traído del catálogo.");
        return;
    }
    if (anotar([&](const std::vector<PerfilTxt>& actual) { return cambio_por_nombre(actual, p, "", false); }))
        estado(L"Perfil \"" + W(nombre) + L"\" agregado con los valores iniciales (barra 6000, despunte 10, zona muerta 230, "
                                          L"separación 3). Corrígelos en la tabla de perfiles si hace falta.");
}

// ---------------------------------------------------------------------------
// Resultado

void llenar_resultado_tabla(HWND lv, const std::vector<std::vector<std::string>>& f) {
    SendMessageW(lv, WM_SETREDRAW, FALSE, 0);
    ListView_DeleteAllItems(lv);
    for (size_t i = 0; i < f.size(); i++) {
        LVITEMW it{};
        it.mask = LVIF_TEXT;
        it.iItem = (int)i;
        std::wstring t0 = W(f[i][0]);
        it.pszText = const_cast<wchar_t*>(t0.c_str());
        ListView_InsertItem(lv, &it);
        for (size_t c = 1; c < f[i].size(); c++) poner_texto(lv, (int)i, (int)c, W(f[i][c]));
    }
    SendMessageW(lv, WM_SETREDRAW, TRUE, 0);
    InvalidateRect(lv, nullptr, TRUE);
}

void mostrar_plan(int k) {
    if (k < 0 || k >= (int)app.resultados.size()) {
        ListView_DeleteAllItems(app.plan);
        SetDlgItemTextW(app.wnd, ID_ET_PLAN, L"Plan por barra");
        return;
    }
    const ResultadoPerfil& r = app.resultados[k];
    auto f = filas_plan(r);
    app.plan_sombra.assign(f.size(), 0);
    char sombra = 1;
    for (size_t i = 0; i < f.size(); i++) {
        if (!f[i][0].empty()) sombra = !sombra;
        app.plan_sombra[i] = sombra;
    }
    llenar_resultado_tabla(app.plan, f);
    std::wstring et = L"Plan por barra: " + W(r.prob.nombre) + L"  (barra " + W(medida(r.prob.param.largo_barra, r.prob.escala)) +
                      L", útil " + W(medida(r.prob.param.util(), r.prob.escala)) + L" mm)";
    SetDlgItemTextW(app.wnd, ID_ET_PLAN, et.c_str());
}

void mostrar_resultados() {
    std::vector<std::vector<std::string>> f;
    for (const auto& r : app.resultados) f.push_back(fila_resumen(r));
    llenar_resultado_tabla(app.resumen, f);
    if (!app.resultados.empty()) ListView_SetItemState(app.resumen, 0, LVIS_SELECTED | LVIS_FOCUSED, LVIS_SELECTED | LVIS_FOCUSED);
    mostrar_plan(app.resultados.empty() ? -1 : 0);
}

void botones() {
    EnableWindow(GetDlgItem(app.wnd, ID_CALCULAR), !app.calculando);
    EnableWindow(GetDlgItem(app.wnd, ID_CANCELAR), app.calculando);
    for (int id : {ID_NUEVO, ID_ABRIR, ID_GUARDAR, ID_GUARDAR_COMO, ID_QUITAR_PIEZA, ID_QUITAR_PERFIL})
        EnableWindow(GetDlgItem(app.wnd, id), !app.calculando);
    EnableWindow(GetDlgItem(app.wnd, ID_EXPORTAR), !app.calculando && !app.resultados.empty());
    EnableWindow(GetDlgItem(app.wnd, ID_CARPETA_DATOS), !app.calculando && !app.examinando);
    EnableWindow(app.perfiles, !app.calculando);
    EnableWindow(app.piezas, !app.calculando);
}

void calcular() {
    cerrar_edicion(false);
    if (app.calculando) return;
    std::vector<ProblemaPerfil> problemas;
    std::vector<std::string> errores;
    if (!preparar(app.trabajo, problemas, errores)) {
        std::wstring msg = L"Corrige esto antes de calcular:\n\n";
        for (size_t i = 0; i < errores.size() && i < 15; i++) msg += L"- " + W(errores[i]) + L"\n";
        if (errores.size() > 15) msg += L"... y " + std::to_wstring(errores.size() - 15) + L" más.\n";
        estado(std::to_wstring(errores.size()) + (errores.size() == 1 ? L" dato por corregir." : L" datos por corregir."));
        cuadro(msg.c_str(), MB_ICONWARNING);
        return;
    }
    if (app.hilo.joinable()) app.hilo.join();
    app.calculando = true;
    app.cancelar = false;
    botones();
    app.resultados.clear();
    mostrar_resultados();
    estado(L"Calculando...");
    HWND wnd = app.wnd;
    std::atomic<bool>* cancelar = &app.cancelar;
    app.hilo = std::thread([wnd, cancelar, problemas]() {
        auto res = std::make_unique<Resultado>();
        for (size_t k = 0; k < problemas.size(); k++) {
            PostMessageW(wnd, WM_APP_PROGRESO, k, problemas.size());
            Control ctl;
            ctl.cancelar = cancelar;
            ctl.limite = std::chrono::steady_clock::now() + std::chrono::seconds(TOPE_SEGUNDOS);
            res->perfiles.push_back(resolver(problemas[k], &ctl));
        }
        PostMessageW(wnd, WM_APP_FIN, 0, (LPARAM)res.release());
    });
}

void fin_calculo(Resultado* res) {
    std::unique_ptr<Resultado> r(res);
    if (app.hilo.joinable()) app.hilo.join();
    app.calculando = false;
    app.resultados = std::move(r->perfiles);
    app.resultado_viejo = false;
    botones();
    mostrar_resultados();
    long long total = 0;
    int no_dem = 0, errores = 0, cortados = 0, no_caben = 0;
    for (const auto& x : app.resultados) {
        if (!x.valido) { errores++; continue; }
        total += x.barras_enviar();
        no_dem += !x.plan.demostrado;
        cortados += x.plan.estado != Estado::completo;
        no_caben += (int)x.plan.piezas_no_caben;
    }
    std::wstring t = L"Listo: " + std::to_wstring(total) + L" barras a enviar en " + std::to_wstring(app.resultados.size()) +
                     (app.resultados.size() == 1 ? L" perfil." : L" perfiles.");
    if (errores) t += L" " + std::to_wstring(errores) + L" con error.";
    if (no_dem) t += L" Mínimo no demostrado en " + std::to_wstring(no_dem) + L".";
    else if (!errores) t += L" Mínimo demostrado en todos.";
    if (cortados) t += L" " + std::to_wstring(cortados) + L" cortado(s) por tiempo o cancelación.";
    if (no_caben) t += L" " + std::to_wstring(no_caben) + L" pieza(s) no caben: ver el plan.";
    estado(t);
}

// ---------------------------------------------------------------------------
// Archivos

// Dialogo de Abrir o Guardar. Si la carpeta es de red y no responde, empieza en
// su copia de este equipo (lo que se guarde ahi se copia cuando vuelva la red).
// Devuelve siempre el destino real, nunca una ruta de la carpeta de espera.
bool dialogo_archivo(bool guardar, std::wstring& ruta, bool* desde_espera = nullptr) {
    if (desde_espera) *desde_espera = false;
    std::wstring inicial = datos::leer_config(L"carpeta");
    if (inicial.empty()) inicial = datos::leer_config(L"carpeta_datos");
    std::wstring sugerido = ruta, sin_red;   // sin_red: el dialogo empieza en la copia de esta carpeta de red
    if (!sugerido.empty() && fuera_de_linea(sugerido)) {
        sin_red = datos::carpeta_de(sugerido);
        sugerido = datos::ruta_en_espera(sugerido);
        datos::crear_carpetas(datos::carpeta_de(sugerido));
    }
    if (!inicial.empty() && fuera_de_linea(inicial)) {
        if (sugerido.empty()) sin_red = inicial;   // con un archivo sugerido, el dialogo empieza en su carpeta
        inicial = datos::carpeta_en_espera(inicial);
        datos::crear_carpetas(inicial);
    }
    std::wstring titulo_dlg = guardar ? L"Sin conexión con " + sin_red + L": lo que guardes aquí se copiará cuando vuelva la red"
                                      : L"Sin conexión con " + sin_red + L": se ven los trabajos guardados en este equipo";
    for (;;) {
        wchar_t buf[MAX_PATH] = L"";
        if (!sugerido.empty()) lstrcpynW(buf, sugerido.c_str(), MAX_PATH);
        OPENFILENAMEW o{};
        o.lStructSize = sizeof(o);
        o.hwndOwner = app.wnd;
        o.lpstrFilter = L"Trabajos de NestTubo (*.ntb)\0*.ntb\0Todos los archivos\0*.*\0";
        o.lpstrFile = buf;
        o.nMaxFile = MAX_PATH;
        o.lpstrDefExt = L"ntb";
        o.lpstrInitialDir = inicial.empty() ? nullptr : inicial.c_str();
        o.lpstrTitle = sin_red.empty() ? nullptr : titulo_dlg.c_str();
        o.Flags = guardar ? (OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST) : (OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST);
        app.modal++;
        bool ok = guardar ? GetSaveFileNameW(&o) : GetOpenFileNameW(&o);
        app.modal--;
        revisar_pendientes();
        if (!ok) return false;
        ruta = buf;
        std::wstring destino;
        if (datos::destino_en_espera(ruta, destino) && !datos::raiz(destino).empty()) {
            ruta = destino;
            if (desde_espera) *desde_espera = true;   // el dialogo estaba en la copia de este equipo, no en la carpeta real
        } else if (guardar && datos::dentro_de_espera(ruta)) {
            // fuera de las carpetas que repiten una de red no hay a donde copiarlo
            cuadro(L"Esa carpeta no corresponde a ninguna carpeta de red: lo que se guarde ahí no se copia a ningún lado.\n\n"
                   L"Elige la carpeta del trabajo dentro de la que abrió el diálogo, o una carpeta de este equipo.",
                   MB_ICONWARNING);
            sugerido = ruta;
            continue;
        }
        if (guardar && datos::es_remota(ruta) && !datos::destino_valido(ruta)) {
            cuadro(L"Esa carpeta de red no sirve para guardar un trabajo: falta la carpeta compartida.\n\n"
                   L"Elige una carpeta dentro de un recurso compartido (\\\\servidor\\recurso\\...) o una carpeta de este equipo.",
                   MB_ICONWARNING);
            sugerido = ruta;
            continue;
        }
        datos::escribir_config(L"carpeta", datos::carpeta_de(ruta));
        return true;
    }
}

bool guardar(bool como) {
    cerrar_edicion(false);
    std::wstring abierto = app.archivo;   // antes del dialogo: la pasada puede cambiar app.archivo durante el
    std::wstring ruta = app.archivo;
    bool desde_espera = false;
    if (como || ruta.empty()) {
        if (!dialogo_archivo(true, ruta, &desde_espera)) return false;
    }
    bool es_el_abierto = !ruta.empty() && datos::clave_ruta(ruta) == datos::clave_ruta(abierto);
    // la base se decide despues del dialogo. Si el dialogo mostro la copia de este
    // equipo (sin conexion), no se vio la carpeta real: tratar como nuevo para no
    // pisar lo que haya alla. Si es el archivo abierto, lo ultimo que sabe la ventana.
    datos::Sello base;
    if (es_el_abierto) base = app.sello;
    else if (desde_espera || fuera_de_linea(ruta)) base = datos::sello_nuevo();
    else base = datos::sello_de(ruta);
    bool en_espera = false;
    std::wstring error;
    datos::Sello sello;
    if (!datos::guardar_trabajo(ruta, a_texto(app.trabajo), base, es_el_abierto, en_espera, sello, error)) {
        cuadro(error, MB_ICONERROR);
        return false;
    }
    app.archivo = ruta;
    app.sello = sello;
    app.modificado = false;
    app.perfiles_sin_anotar = false;
    titulo();
    if (!en_espera) {
        estado(L"Guardado en " + ruta);
        return true;
    }
    app.trabajos_en_espera = std::max(app.trabajos_en_espera, 1);
    Conexion c = conexion_de(ruta);
    if (c != Conexion::en_linea) {
        std::wstring r = datos::raiz(ruta);
        auto it = std::find_if(app.esperando_red.begin(), app.esperando_red.end(),
                               [&](const std::pair<std::wstring, int>& x) { return datos::clave_ruta(x.first) == datos::clave_ruta(r); });
        if (it == app.esperando_red.end()) app.esperando_red.push_back({r, 1});
    }
    datos::pedir_pasada(ruta);
    estado(c == Conexion::en_linea ? L"Guardado en este equipo; copiando a " + ruta + L"..."
                                   : L"Guardado en este equipo; se copiará a " + ruta + L" cuando haya conexión.");
    etiqueta_datos();
    if (c != Conexion::en_linea && !app.aviso_sin_conexion) {
        app.aviso_sin_conexion = true;
        std::wstring m = (c == Conexion::sin_conexion ? L"No hay conexión con " : L"Todavía no responde ") + datos::carpeta_de(ruta) +
                         L".\n\nEl trabajo quedó guardado en este equipo y se copiará solo cuando haya conexión, "
                         L"con NestTubo abierto. Si en la red ya hay otro archivo con ese nombre, no se reemplaza: "
                         L"este se copia al lado como \"(guardado sin conexión)\".";
        cuadro(m, MB_ICONINFORMATION);
    }
    return true;
}

// true si se puede seguir (no habia cambios, se guardaron o se descartaron)
bool puede_descartar() {
    cerrar_edicion(false);
    if (!app.modificado) return true;
    // sin archivo ni piezas no hay nada que perder, salvo un cambio de perfiles que no quedo en el catalogo
    bool piezas = std::any_of(app.trabajo.piezas.begin(), app.trabajo.piezas.end(), [](const PiezaTxt& p) { return !fila_vacia(p); });
    if (app.archivo.empty() && !piezas && !app.perfiles_sin_anotar) return true;
    int r = cuadro(L"¿Guardar los cambios del trabajo actual?", MB_YESNOCANCEL | MB_ICONQUESTION);
    if (r == IDCANCEL) return false;
    if (r == IDYES) return guardar(false);
    return true;
}

// ---------------------------------------------------------------------------
// Exportar: PDF, Excel y un DXF por distribucion de barra, a una carpeta.

int CALLBACK carpeta_inicial(HWND w, UINT msg, LPARAM, LPARAM dato) {
    if (msg == BFFM_INITIALIZED && dato) SendMessageW(w, BFFM_SETSELECTIONW, TRUE, dato);
    return 0;
}

// Dialogo de carpeta de Windows 10/11 (el del Explorador). Devuelve false si no
// se pudo crear: entonces se usa el arbol viejo.
bool carpeta_moderna(const wchar_t* titulo_dlg, const std::wstring& inicial, std::wstring& carpeta, bool& elegida) {
    IFileOpenDialog* d = nullptr;
    if (FAILED(CoCreateInstance(CLSID_FileOpenDialog, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&d)))) return false;
    DWORD op = 0;
    d->GetOptions(&op);
    d->SetOptions(op | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST);
    d->SetTitle(titulo_dlg);
    IShellItem* ini = nullptr;
    if (!inicial.empty() && SUCCEEDED(SHCreateItemFromParsingName(inicial.c_str(), nullptr, IID_PPV_ARGS(&ini)))) {
        d->SetFolder(ini);
        ini->Release();
    }
    elegida = false;
    IShellItem* r = nullptr;
    if (SUCCEEDED(d->Show(app.wnd)) && SUCCEEDED(d->GetResult(&r))) {
        PWSTR p = nullptr;
        if (SUCCEEDED(r->GetDisplayName(SIGDN_FILESYSPATH, &p))) {
            carpeta = p;
            elegida = true;
            CoTaskMemFree(p);
        }
        r->Release();
    }
    d->Release();
    return true;
}

// Si la carpeta inicial es de red y no responde se empieza en Documentos: pedirle
// la ruta a Windows colgaria la ventana.
bool elegir_carpeta_(const wchar_t* titulo_dlg, std::wstring inicial, std::wstring& carpeta);

bool elegir_carpeta(const wchar_t* titulo_dlg, std::wstring inicial, std::wstring& carpeta) {
    app.modal++;
    bool r = elegir_carpeta_(titulo_dlg, inicial, carpeta);
    app.modal--;
    revisar_pendientes();
    return r;
}

bool elegir_carpeta_(const wchar_t* titulo_dlg, std::wstring inicial, std::wstring& carpeta) {
    if (inicial.empty() || fuera_de_linea(inicial)) inicial = datos::carpeta_documentos();
    bool elegida = false;
    if (carpeta_moderna(titulo_dlg, inicial, carpeta, elegida)) return elegida;
    BROWSEINFOW b{};
    b.hwndOwner = app.wnd;
    b.lpszTitle = titulo_dlg;
    b.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;
    b.lpfn = carpeta_inicial;
    b.lParam = (LPARAM)inicial.c_str();
    PIDLIST_ABSOLUTE id = SHBrowseForFolderW(&b);
    if (!id) return false;
    wchar_t ruta[MAX_PATH];
    bool ok = SHGetPathFromIDListW(id, ruta);
    CoTaskMemFree(id);
    if (!ok) return false;
    carpeta = ruta;
    return true;
}

using datos::existe;

void exportar() {
    cerrar_edicion(false);
    if (app.calculando) return;
    if (app.resultados.empty()) {
        cuadro(L"Primero calcula las barras: se exporta el último cálculo.", MB_ICONINFORMATION);
        return;
    }
    if (app.resultado_viejo) {
        cuadro(L"Los datos cambiaron desde el último cálculo. Calcula de nuevo antes de exportar.", MB_ICONWARNING);
        return;
    }
    std::wstring carpeta, inicial = datos::leer_config(L"carpeta_exportar");
    if (inicial.empty()) inicial = datos::leer_config(L"carpeta");
    if (!elegir_carpeta(L"Carpeta donde dejar el PDF, el Excel y los DXF", inicial, carpeta)) return;
    datos::escribir_config(L"carpeta_exportar", carpeta);
    // nombre del trabajo: el del archivo, o "Trabajo" si no se ha guardado
    std::wstring nombre = L"Trabajo";
    if (!app.archivo.empty()) {
        size_t k = app.archivo.find_last_of(L"\\/");
        nombre = k == std::wstring::npos ? app.archivo : app.archivo.substr(k + 1);
        size_t p = nombre.find_last_of(L'.');
        if (p != std::wstring::npos && p > 0) nombre = nombre.substr(0, p);
    }
    if (!carpeta.empty() && carpeta.back() != L'\\') carpeta += L'\\';
    std::wstring pdf = carpeta + nombre + L" - plan.pdf", xlsx = carpeta + nombre + L" - plan.xlsx",
                 dxf = carpeta + nombre + L" - DXF";
    if (existe(pdf) || existe(xlsx) || existe(dxf)) {
        std::wstring q = L"Ya hay una exportación de \"" + nombre + L"\" en esa carpeta.\n\n¿Reemplazarla? Se borran los DXF de la carpeta \"" +
                         nombre + L" - DXF\" para que no se mezclen los viejos con los nuevos.";
        if (cuadro(q.c_str(), MB_YESNO | MB_ICONQUESTION) != IDYES) return;
        WIN32_FIND_DATAW f;
        HANDLE h = FindFirstFileW((dxf + L"\\*.dxf").c_str(), &f);
        if (h != INVALID_HANDLE_VALUE) {
            do DeleteFileW((dxf + L"\\" + f.cFileName).c_str());
            while (FindNextFileW(h, &f));
            FindClose(h);
        }
    }
    SYSTEMTIME t;
    GetLocalTime(&t);
    char fecha[16];
    std::snprintf(fecha, sizeof fecha, "%02d/%02d/%04d", t.wDay, t.wMonth, t.wYear);
    std::string trabajo = U(nombre);
    estado(L"Exportando...");
    std::wstring falla;
    if (!datos::escribir_archivo(pdf, informe_pdf(app.resultados, trabajo, fecha))) falla = pdf;
    else if (!datos::escribir_archivo(xlsx, informe_xlsx(app.resultados, trabajo, fecha))) falla = xlsx;
    int n_dxf = 0;
    if (falla.empty()) {
        CreateDirectoryW(dxf.c_str(), nullptr);
        for (const auto& r : app.resultados) {
            for (const auto& d : dxf_perfil(r)) {
                std::wstring ruta = dxf + L"\\" + W(d.nombre);
                if (!datos::escribir_archivo(ruta, d.texto)) { falla = ruta; break; }
                n_dxf++;
            }
            if (!falla.empty()) break;
        }
    }
    if (!falla.empty()) {
        estado(L"No se pudo exportar.");
        cuadro((L"No se pudo escribir\n" + falla + L"\n\n¿Está abierto en otro programa? Ciérralo y vuelve a exportar.").c_str(), MB_ICONERROR);
        return;
    }
    estado(L"Exportado en " + carpeta + L": PDF, Excel y " + std::to_wstring(n_dxf) + L" DXF.");
    std::wstring msg = L"Listo. En " + carpeta + L" quedaron:\n\n" + nombre + L" - plan.pdf\n" + nombre + L" - plan.xlsx\n" +
                       nombre + L" - DXF  (" + std::to_wstring(n_dxf) + (n_dxf == 1 ? L" archivo)" : L" archivos)") +
                       L"\n\n¿Abrir la carpeta?";
    if (cuadro(msg.c_str(), MB_YESNO | MB_ICONINFORMATION) == IDYES)
        ShellExecuteW(app.wnd, L"open", carpeta.c_str(), nullptr, nullptr, SW_SHOWNORMAL);
}

void limpiar_resultado() {
    app.resultados.clear();
    app.resultado_viejo = false;
    mostrar_resultados();
    botones();
}

void nuevo() {
    if (!puede_descartar()) return;
    app.trabajo.piezas.clear();
    app.catalogo = datos::catalogo_vigente(app.catalogo);
    app.trabajo.perfiles = app.catalogo;
    asegurar_fila_vacia();
    app.archivo.clear();
    app.sello = datos::sello_nuevo();
    app.modificado = false;
    app.perfiles_sin_anotar = false;
    llenar_tabla(app.perfiles);
    llenar_tabla(app.piezas);
    limpiar_resultado();
    titulo();
    estado(L"Trabajo nuevo con los perfiles del catálogo.");
}

// Sin ruta pregunta con el dialogo; con ruta (la que llega al arrastrar un
// .ntb sobre el programa o al usar "Abrir con") la abre directo.
void abrir(std::wstring ruta = L"") {
    if (!puede_descartar()) return;
    if (ruta.empty() && !dialogo_archivo(false, ruta)) return;
    std::wstring destino;
    bool en_espera = false;
    datos::Sello base;
    std::wstring leer = datos::version_a_abrir(ruta, destino, en_espera, base);
    std::string texto, error;
    DWORD cod = 0;
    Trabajo t;
    bool ok = datos::leer_archivo(leer, texto, &cod) == Lectura::ok;
    if (!ok) error = U(datos::texto_error(cod));
    else ok = de_texto(texto, t, error);
    if (!ok) {
        cuadro((L"No se pudo abrir " + leer + L"\n\n" + W(error)).c_str(), MB_ICONERROR);
        return;
    }
    app.trabajo = t;
    asegurar_fila_vacia();
    app.archivo = destino;
    app.sello = en_espera ? base : datos::sello_de(leer);
    if (!en_espera && app.sello.existe) datos::recordar(destino, app.sello);
    app.modificado = false;
    app.perfiles_sin_anotar = false;
    llenar_tabla(app.perfiles);
    llenar_tabla(app.piezas);
    limpiar_resultado();
    titulo();
    if (en_espera) estado(L"Abierta la copia guardada en este equipo, que aún no llega a " + destino + L".");
    else estado(L"Abierto " + destino);
    datos::pedir_pasada(app.archivo);
}

void quitar_filas(HWND lv) {
    cerrar_edicion(false);
    std::vector<int> sel;
    for (int i = ListView_GetNextItem(lv, -1, LVNI_SELECTED); i >= 0; i = ListView_GetNextItem(lv, i, LVNI_SELECTED))
        if (i < filas(lv) - 1) sel.push_back(i);   // la ultima fila (vacia) no se quita
    if (sel.empty()) {
        estado(L"Selecciona primero las filas que quieres quitar.");
        return;
    }
    std::wstring q = !es_perfiles(lv) ? L"¿Quitar " + cuantos((int)sel.size(), L"fila de piezas", L"filas de piezas") + L"?"
                     : sel.size() == 1 ? L"¿Quitar el perfil? También se quita del catálogo y no saldrá en los trabajos nuevos."
                                       : L"¿Quitar " + std::to_wstring(sel.size()) +
                                             L" perfiles? También se quitan del catálogo y no saldrán en los trabajos nuevos.";
    if (cuadro(q.c_str(), MB_YESNO | MB_ICONQUESTION) != IDYES) return;
    std::vector<std::string> nombres;
    for (size_t k = sel.size(); k-- > 0;) {
        if (es_perfiles(lv)) {
            nombres.push_back(app.trabajo.perfiles[sel[k]].nombre);
            app.trabajo.perfiles.erase(app.trabajo.perfiles.begin() + sel[k]);
        } else {
            app.trabajo.piezas.erase(app.trabajo.piezas.begin() + sel[k]);
        }
    }
    CambiosCatalogo quitados;   // solo si ninguna fila que queda tiene ese perfil
    for (const auto& n : nombres)
        if (!clave_perfil(n).empty() && buscar_perfil(app.trabajo.perfiles, n) < 0) quitados.quitados.push_back(clave_perfil(n));
    if (!quitados.vacio() && anotar([&](const std::vector<PerfilTxt>&) { return quitados; }))
        estado(quitados.quitados.size() == 1 ? L"Perfil quitado del catálogo." : L"Perfiles quitados del catálogo.");
    asegurar_fila_vacia();
    llenar_tabla(lv);
    datos_cambiaron();
}

// ---------------------------------------------------------------------------
// Carpeta de datos

void elegir_carpeta_datos() {
    cerrar_edicion(false);
    if (app.examinando) return;
    std::wstring actual = datos::leer_config(L"carpeta_datos"), f;
    if (!elegir_carpeta(L"Carpeta de datos de NestTubo (ahí se guarda el catálogo de perfiles)", actual, f)) return;
    if (datos::clave_ruta(f) == datos::clave_ruta(actual)) {
        estado(L"Esa ya es la carpeta de datos.");
        return;
    }
    app.examinando = true;
    botones();
    estado(L"Revisando " + f + L"...");
    etiqueta_datos();
    datos::examinar(f);
}

// El hilo ya leyo el catalogo de la carpeta elegida: se junta con el de este equipo.
void carpeta_examinada(const datos::InformeExaminar& inf) {
    app.examinando = false;
    botones();
    etiqueta_datos();
    const std::wstring& f = inf.carpeta;
    if (inf.lectura == Lectura::error) {
        estado(L"La carpeta de datos no cambió.");
        cuadro((L"No se pudo leer " + f + L"\\perfiles.ntb: " + inf.error + L".\n\nLa carpeta de datos no cambió.").c_str(), MB_ICONWARNING);
        return;
    }
    bool habia = inf.lectura == Lectura::ok, gana_carpeta = true;
    if (habia) {
        std::vector<std::string> d = perfiles_distintos(inf.catalogo, datos::catalogo_vigente(app.catalogo));
        if (!d.empty()) {
            std::wstring lista;
            for (size_t i = 0; i < d.size() && i < 15; i++) lista += L"   " + W(d[i]) + L"\n";
            if (d.size() > 15) lista += L"   ... y " + std::to_wstring(d.size() - 15) + L" más\n";
            std::wstring q = L"Estos perfiles tienen valores distintos en la carpeta y en este equipo:\n\n" + lista +
                             L"\n¿Usar los de la carpeta?\n\nSí = los de la carpeta.\nNo = los de este equipo.\nCancelar = no cambiar de carpeta.";
            int r = cuadro(q, MB_YESNOCANCEL | MB_ICONQUESTION);
            if (r == IDCANCEL) {
                estado(L"La carpeta de datos no cambió.");
                return;
            }
            gana_carpeta = r == IDYES;
        }
    }
    // la mezcla se hace con la copia local de ahora (pudo cambiar mientras estaba la pregunta)
    std::wstring error;
    std::vector<PerfilTxt> resultado;
    if (!datos::cambiar_carpeta(f, inf.lectura, inf.catalogo, gana_carpeta, resultado, error)) {
        cuadro(error + L"\n\nLa carpeta de datos no cambió.", MB_ICONERROR);
        return;
    }
    app.catalogo = resultado;
    app.catalogo_en_espera = true;
    app.error_catalogo.clear();
    app.conexion[datos::clave_ruta(datos::raiz(f))] = Conexion::en_linea;
    bool piezas = std::any_of(app.trabajo.piezas.begin(), app.trabajo.piezas.end(), [](const PiezaTxt& p) { return !fila_vacia(p); });
    if (app.archivo.empty() && !piezas) {   // nada abierto: la tabla muestra el catalogo nuevo
        app.trabajo.perfiles = app.catalogo;
        asegurar_fila_vacia();
        llenar_tabla(app.perfiles);
    }
    datos::pedir_pasada(app.archivo);
    estado(L"Carpeta de datos: " + f + (habia ? L". Se juntó su catálogo de perfiles con el de este equipo." : L". Se copia ahí el catálogo de perfiles."));
    etiqueta_datos();
}

// Lo que llego del hilo y abre cuadros, cuando la ventana esta libre.
void atender_pendientes() {
    if (ocupado()) return;
    if (app.examen) {
        std::unique_ptr<datos::InformeExaminar> inf = std::move(app.examen);
        carpeta_examinada(*inf);
    }
    while (!ocupado() && !app.avisos.empty()) {
        std::wstring m = app.avisos.front();
        app.avisos.erase(app.avisos.begin());
        cuadro(m, MB_ICONWARNING);
    }
}

// Lo que informa el hilo despues de cada pasada. No abre cuadros: los deja en app.avisos.
void pasada_hecha(const datos::InformePasada& inf) {
    if (inf.saltada) {
        // otro proceso hizo la pasada: su informe no trae estos datos. Se deja el ultimo completo.
        etiqueta_datos();
        return;
    }
    for (const auto& r : inf.raices) app.conexion[datos::clave_ruta(r.first)] = r.second;
    app.trabajos_en_espera = inf.trabajos_en_espera;
    app.catalogo_en_espera = inf.catalogo_en_espera;
    app.esperando_red = inf.esperando_red;
    // la copia local se puso al dia: se lee ahora (la del informe pudo quedar vieja frente a una edicion reciente)
    if (inf.hay_catalogo && datos::clave_ruta(inf.carpeta_datos) == datos::clave_ruta(datos::leer_config(L"carpeta_datos")))
        app.catalogo = datos::catalogo_vigente(app.catalogo);   // la tabla no se toca: es la del trabajo abierto
    app.error_catalogo = inf.error_catalogo;
    app.fallas = (int)inf.fallas.size();
    for (const auto& c : inf.copiados) {
        bool al_lado = datos::clave_ruta(c.escrito) != datos::clave_ruta(c.destino), abierto = false;
        if (datos::clave_ruta(c.destino) == datos::clave_ruta(app.archivo)) {
            if (!al_lado) {
                app.sello = c.sello;
            } else if (!c.sigue_en_espera) {   // el trabajo abierto ahora es el que quedo al lado
                app.archivo = c.escrito;
                app.sello = c.sello;
                abierto = true;
                titulo();
            }
        }
        if (al_lado)
            app.avisos.push_back(L"En la red ya había un archivo\n" + c.destino +
                                 L"\nque no es el que este equipo conocía (otro trabajo con el mismo nombre, o cambió mientras no había "
                                 L"conexión). No se reemplazó: lo que guardaste sin conexión quedó como\n" + c.escrito +
                                 (abierto ? L"\n\nEl trabajo abierto ahora es ese archivo." : L"") + L"\n\nRevisa los dos y borra el que sobre.");
    }
    if (!inf.copiados.empty()) {
        const auto& u = inf.copiados.back();
        estado(inf.copiados.size() == 1 ? L"Copiado a " + u.escrito
                                        : L"Copiados " + std::to_wstring(inf.copiados.size()) + L" trabajos; el último a " + u.escrito);
    }
    etiqueta_datos();
    // errores reales (la red respondia): un cuadro por trabajo y codigo
    for (const auto& f : inf.fallas) {
        std::wstring k = datos::clave_ruta(f.en_espera) + L"|" + std::to_wstring(f.codigo);
        if (app.fallas_avisadas.count(k)) continue;
        app.fallas_avisadas.insert(k);
        app.avisos.push_back(L"No se pudo copiar el trabajo a\n" + f.destino + L"\n\n" + datos::texto_error(f.codigo) +
                             L"\n\nQuedó guardado en este equipo en\n" + f.en_espera + L"\n\nNestTubo lo vuelve a intentar cada 30 segundos.");
    }
    atender_pendientes();
}

// ---------------------------------------------------------------------------
// Ventana

HWND crear_lista(int id, const wchar_t* const* cols, const int* anchos, int n, bool rejilla) {
    HWND lv = CreateWindowExW(WS_EX_CLIENTEDGE, WC_LISTVIEWW, L"", WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_CLIPCHILDREN | LVS_REPORT | LVS_SHOWSELALWAYS,
                              0, 0, 10, 10, app.wnd, (HMENU)(INT_PTR)id, app.inst, nullptr);
    SendMessageW(lv, WM_SETFONT, (WPARAM)app.fuente, TRUE);
    ListView_SetExtendedListViewStyle(lv, LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER | LVS_EX_INFOTIP | (rejilla ? LVS_EX_GRIDLINES : 0));
    for (int i = 0; i < n; i++) {
        LVCOLUMNW c{};
        c.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_FMT;
        c.fmt = LVCFMT_LEFT;
        c.cx = S(anchos[i]);
        c.pszText = const_cast<wchar_t*>(cols[i]);
        ListView_InsertColumn(lv, i, &c);
    }
    return lv;
}

HWND crear_boton(int id, const wchar_t* t) {
    HWND b = CreateWindowExW(0, L"BUTTON", t, WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON, 0, 0, 10, 10, app.wnd,
                             (HMENU)(INT_PTR)id, app.inst, nullptr);
    SendMessageW(b, WM_SETFONT, (WPARAM)app.fuente, TRUE);
    return b;
}

HWND crear_etiqueta(int id, const wchar_t* t, HFONT f) {
    HWND e = CreateWindowExW(0, L"STATIC", t, WS_CHILD | WS_VISIBLE | SS_LEFT | SS_CENTERIMAGE, 0, 0, 10, 10, app.wnd,
                             (HMENU)(INT_PTR)id, app.inst, nullptr);
    SendMessageW(e, WM_SETFONT, (WPARAM)f, TRUE);
    return e;
}

void crear_controles() {
    NONCLIENTMETRICSW nm{};
    nm.cbSize = sizeof(nm);
    SystemParametersInfoW(SPI_GETNONCLIENTMETRICS, sizeof(nm), &nm, 0);
    LOGFONTW lf = nm.lfMessageFont;
    lf.lfHeight = -S(14);
    app.fuente = CreateFontIndirectW(&lf);
    lf.lfWeight = FW_BOLD;
    app.negrita = CreateFontIndirectW(&lf);

    crear_boton(ID_NUEVO, L"Nuevo");
    crear_boton(ID_ABRIR, L"Abrir...");
    crear_boton(ID_GUARDAR, L"Guardar");
    crear_boton(ID_GUARDAR_COMO, L"Guardar como...");
    crear_boton(ID_CALCULAR, L"Calcular barras");
    crear_boton(ID_CANCELAR, L"Cancelar");
    crear_boton(ID_EXPORTAR, L"Exportar PDF, Excel y DXF...");
    crear_boton(ID_CARPETA_DATOS, L"Carpeta de datos...");
    crear_boton(ID_QUITAR_PERFIL, L"Quitar perfil");
    crear_boton(ID_QUITAR_PIEZA, L"Quitar fila");
    crear_etiqueta(ID_ET_PERFILES, L"Perfiles  (medidas en mm; margen = barras extra a enviar)", app.negrita);
    crear_etiqueta(ID_ET_PIEZAS, L"Piezas del pedido  (ángulo 0 = corte recto)", app.negrita);
    crear_etiqueta(ID_ET_RESUMEN, L"Barras a enviar por perfil", app.negrita);
    crear_etiqueta(ID_ET_PLAN, L"Plan por barra", app.negrita);

    app.perfiles = crear_lista(ID_PERFILES, COLS_PERFILES, ANCHO_PERFILES, 7, true);
    app.piezas = crear_lista(ID_PIEZAS, COLS_PIEZAS, ANCHO_PIEZAS, 6, true);
    std::vector<std::string> cr = columnas_resumen(), cp = columnas_plan();
    std::vector<std::wstring> wr, wp;
    for (auto& s : cr) wr.push_back(W(s));
    for (auto& s : cp) wp.push_back(W(s));
    std::vector<const wchar_t*> pr, pp;
    for (auto& s : wr) pr.push_back(s.c_str());
    for (auto& s : wp) pp.push_back(s.c_str());
    const int ancho_res[] = {125, 55, 115, 60, 55, 85, 65, 120};
    const int ancho_plan[] = {110, 170, 75, 75, 75, 180};
    app.resumen = crear_lista(ID_RESUMEN, pr.data(), ancho_res, (int)pr.size(), false);
    app.plan = crear_lista(ID_PLAN, pp.data(), ancho_plan, (int)pp.size(), false);

    app.estado = CreateWindowExW(0, STATUSCLASSNAMEW, L"", WS_CHILD | WS_VISIBLE | SBARS_SIZEGRIP, 0, 0, 0, 0, app.wnd,
                                 (HMENU)ID_ESTADO, app.inst, nullptr);
    SendMessageW(app.estado, WM_SETFONT, (WPARAM)app.fuente, TRUE);
    botones();
}

void acomodar() {
    RECT r;
    GetClientRect(app.wnd, &r);
    SendMessageW(app.estado, WM_SIZE, 0, 0);
    RECT rs;
    GetWindowRect(app.estado, &rs);
    int W_ = r.right, H = r.bottom - (rs.bottom - rs.top);
    int m = S(10), hb = S(30), he = S(24), sep = S(8);
    auto mover = [](int id, int x, int y, int w, int h) { MoveWindow(GetDlgItem(app.wnd, id), x, y, w, h, TRUE); };
    // barra de botones
    int x = m, y = m;
    int anchos[] = {80, 90, 85, 125};
    int ids[] = {ID_NUEVO, ID_ABRIR, ID_GUARDAR, ID_GUARDAR_COMO};
    for (int i = 0; i < 4; i++) {
        mover(ids[i], x, y, S(anchos[i]), hb);
        x += S(anchos[i]) + S(6);
    }
    x += S(24);
    mover(ID_CALCULAR, x, y, S(150), hb);
    x += S(156);
    mover(ID_CANCELAR, x, y, S(90), hb);
    x += S(96) + S(24);
    mover(ID_EXPORTAR, x, y, S(210), hb);
    x += S(210) + S(24);
    mover(ID_CARPETA_DATOS, x, y, S(150), hb);
    // barra de estado: mensajes a la izquierda, carpeta de datos a la derecha
    int partes[2] = {std::max(S(200), W_ * 52 / 100), -1};
    SendMessageW(app.estado, SB_SETPARTS, 2, (LPARAM)partes);
    SendMessageW(app.estado, SB_SETTEXTW, 1 | SBT_OWNERDRAW, (LPARAM)app.texto_datos.c_str());
    // dos columnas: entrada a la izquierda, resultado a la derecha
    int top = y + hb + sep;
    int ancho = W_ - 3 * m;
    int wl = ancho * 48 / 100, wr = ancho - wl;
    int xl = m, xr = 2 * m + wl;
    int alto = H - top - m;
    int h1 = std::max(S(110), alto * 30 / 100);   // perfiles / resumen
    int y2 = top + he + h1 + sep;
    int h2 = H - m - (y2 + he);
    mover(ID_ET_PERFILES, xl, top, wl - S(130), he);
    mover(ID_QUITAR_PERFIL, xl + wl - S(120), top, S(120), he);
    MoveWindow(app.perfiles, xl, top + he, wl, h1, TRUE);
    mover(ID_ET_RESUMEN, xr, top, wr, he);
    MoveWindow(app.resumen, xr, top + he, wr, h1, TRUE);
    mover(ID_ET_PIEZAS, xl, y2, wl - S(130), he);
    mover(ID_QUITAR_PIEZA, xl + wl - S(120), y2, S(120), he);
    MoveWindow(app.piezas, xl, y2 + he, wl, std::max(h2, S(60)), TRUE);
    mover(ID_ET_PLAN, xr, y2, wr, he);
    MoveWindow(app.plan, xr, y2 + he, wr, std::max(h2, S(60)), TRUE);
}

LRESULT notificacion(NMHDR* nh) {
    HWND lv = nh->hwndFrom;
    bool entrada = lv == app.perfiles || lv == app.piezas;
    switch (nh->code) {
    case NM_CLICK:
    case NM_DBLCLK:
        if (entrada) {
            auto* ia = (NMITEMACTIVATE*)nh;
            LVHITTESTINFO hi{};
            hi.pt = ia->ptAction;
            if (ListView_SubItemHitTest(lv, &hi) >= 0 && hi.iItem >= 0) iniciar_edicion(lv, hi.iItem, hi.iSubItem);
        }
        break;
    case NM_RETURN:
        if (entrada) {
            int f = ListView_GetNextItem(lv, -1, LVNI_FOCUSED);
            iniciar_edicion(lv, f < 0 ? 0 : f, 0);
        }
        break;
    case LVN_KEYDOWN:
        if (entrada) {
            auto* kd = (NMLVKEYDOWN*)nh;
            if (kd->wVKey == VK_F2) {
                int f = ListView_GetNextItem(lv, -1, LVNI_FOCUSED);
                iniciar_edicion(lv, f < 0 ? 0 : f, 0);
            } else if (kd->wVKey == VK_DELETE) {
                quitar_filas(lv);
            }
        }
        break;
    case LVN_BEGINSCROLL:
        if (entrada && app.edit && app.edit_lv == lv) cerrar_edicion(false);
        break;
    case LVN_ITEMCHANGED:
        if (lv == app.resumen) {
            auto* nv = (NMLISTVIEW*)nh;
            if ((nv->uChanged & LVIF_STATE) && (nv->uNewState & LVIS_SELECTED) && !(nv->uOldState & LVIS_SELECTED))
                mostrar_plan(nv->iItem);
        }
        break;
    case NM_CUSTOMDRAW:
        if (lv == app.resumen) {
            auto* cd = (NMLVCUSTOMDRAW*)nh;
            if (cd->nmcd.dwDrawStage == CDDS_PREPAINT) return CDRF_NOTIFYITEMDRAW;
            if (cd->nmcd.dwDrawStage == CDDS_ITEMPREPAINT) return CDRF_NOTIFYSUBITEMDRAW;
            if (cd->nmcd.dwDrawStage == (CDDS_ITEMPREPAINT | CDDS_SUBITEM)) {
                int k = (int)cd->nmcd.dwItemSpec;
                bool barras = cd->iSubItem == 2;
                SelectObject(cd->nmcd.hdc, barras ? app.negrita : app.fuente);
                cd->clrText = GetSysColor(COLOR_WINDOWTEXT);
                if (k < (int)app.resultados.size()) {
                    const auto& r = app.resultados[k];
                    if (!r.valido || (cd->iSubItem == 7 && !r.plan.demostrado)) cd->clrText = RGB(176, 32, 32);
                }
                return CDRF_NEWFONT;
            }
        }
        if (lv == app.plan) {
            auto* cd = (NMLVCUSTOMDRAW*)nh;
            if (cd->nmcd.dwDrawStage == CDDS_PREPAINT) return CDRF_NOTIFYITEMDRAW;
            if (cd->nmcd.dwDrawStage == CDDS_ITEMPREPAINT) {
                size_t k = cd->nmcd.dwItemSpec;
                if (k < app.plan_sombra.size() && app.plan_sombra[k]) cd->clrTextBk = RGB(236, 240, 245);
                return CDRF_DODEFAULT;
            }
        }
        break;
    }
    // las columnas cambian de ancho: el EDIT quedaria fuera de su celda
    if ((nh->code == HDN_BEGINTRACKW || nh->code == HDN_BEGINTRACKA) && app.edit) cerrar_edicion(false);
    return 0;
}

LRESULT CALLBACK Proc(HWND h, UINT m, WPARAM w, LPARAM l) {
    switch (m) {
    case WM_CREATE: {
        app.wnd = h;
        HDC dc = GetDC(h);
        app.dpi = GetDeviceCaps(dc, LOGPIXELSX);
        ReleaseDC(h, dc);
        crear_controles();
        {
            std::wstring error;
            if (!datos::cargar_catalogo(app.catalogo, error)) cuadro(error.c_str(), MB_ICONWARNING);
        }
        app.trabajo.perfiles = app.catalogo;
        app.sello = datos::sello_nuevo();
        asegurar_fila_vacia();
        llenar_tabla(app.perfiles);
        llenar_tabla(app.piezas);
        titulo();
        estado(L"Escribe los perfiles y las piezas del pedido y pulsa \"Calcular barras\". Clic en una celda para editarla.");
        etiqueta_datos();
        datos::iniciar_hilo(h);
        datos::pedir_pasada(L"");
        return 0;
    }
    case WM_SIZE:
        acomodar();
        return 0;
    case WM_GETMINMAXINFO: {
        auto* mm = (MINMAXINFO*)l;
        mm->ptMinTrackSize.x = S(1110);
        mm->ptMinTrackSize.y = S(560);
        return 0;
    }
    case WM_NOTIFY:
        return notificacion((NMHDR*)l);
    case WM_APP_MOVER:
        if (l == 1) cerrar_edicion(true);
        else if (w == 0) {
            // perdio el foco: confirmar, salvo que el foco haya ido a otra celda de la misma edicion
            if (app.edit && GetFocus() != app.edit) cerrar_edicion(false);
        } else mover_edicion((int)w);
        return 0;
    case WM_APP_PROGRESO:
        estado(L"Calculando perfil " + std::to_wstring(w + 1) + L" de " + std::to_wstring(l) + L"...");
        return 0;
    case WM_APP_FIN:
        fin_calculo((Resultado*)l);
        return 0;
    case datos::WM_APP_RED:
        if (w == datos::INFORME_PASADA) {
            std::unique_ptr<datos::InformePasada> inf((datos::InformePasada*)l);
            pasada_hecha(*inf);
        } else if (w == datos::INFORME_EXAMINAR) {
            app.examen.reset((datos::InformeExaminar*)l);
            atender_pendientes();
        }
        return 0;
    case WM_APP_PENDIENTES:
        atender_pendientes();
        return 0;
    case WM_DRAWITEM: {
        auto* di = (DRAWITEMSTRUCT*)l;
        if (di->hwndItem != app.estado) break;
        RECT r = di->rcItem;
        r.left += S(4);
        SetBkMode(di->hDC, TRANSPARENT);
        SetTextColor(di->hDC, app.datos_rojo ? RGB(176, 32, 32) : GetSysColor(COLOR_BTNTEXT));
        HGDIOBJ antes = SelectObject(di->hDC, app.datos_rojo ? app.negrita : app.fuente);
        DrawTextW(di->hDC, app.texto_datos.c_str(), -1, &r, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX);
        SelectObject(di->hDC, antes);
        return TRUE;
    }
    case WM_COMMAND:
        switch (LOWORD(w)) {
        case ID_NUEVO: nuevo(); break;
        case ID_ABRIR: abrir(); break;
        case ID_GUARDAR: guardar(false); break;
        case ID_GUARDAR_COMO: guardar(true); break;
        case ID_CALCULAR: calcular(); break;
        case ID_EXPORTAR: exportar(); break;
        case ID_CARPETA_DATOS: elegir_carpeta_datos(); break;
        case ID_CANCELAR:
            app.cancelar = true;
            estado(L"Cancelando: se muestra lo mejor que se tenga...");
            break;
        case ID_QUITAR_PIEZA: quitar_filas(app.piezas); break;
        case ID_QUITAR_PERFIL: quitar_filas(app.perfiles); break;
        }
        return 0;
    case WM_CTLCOLORSTATIC:
        SetBkMode((HDC)w, TRANSPARENT);
        return (LRESULT)GetSysColorBrush(COLOR_BTNFACE);
    case WM_COPYDATA: {
        auto* cds = (COPYDATASTRUCT*)l;
        if (cds && cds->lpData && cds->cbData >= sizeof(wchar_t)) {
            std::wstring ruta((const wchar_t*)cds->lpData, cds->cbData / sizeof(wchar_t));
            while (!ruta.empty() && ruta.back() == L'\0') ruta.pop_back();
            SetForegroundWindow(h);
            if (!ruta.empty()) abrir(ruta);
        }
        return TRUE;
    }
    case WM_CLOSE:
        if (app.calculando) {
            app.cancelar = true;
            if (app.hilo.joinable()) app.hilo.join();
            app.calculando = false;
        }
        if (!puede_descartar()) return 0;
        if (app.trabajos_en_espera > 0 || app.catalogo_en_espera) {
            // lo que falte ya esta en este equipo; se espera un poco por si alcanza a copiarse
            estado(L"Copiando a la carpeta de datos antes de cerrar...");
            datos::esperar_pasada(datos::pedir_pasada(app.archivo), 3000);
        }
        // mostrar los avisos de la ultima pasada (una copia que quedo "(guardado sin conexión)") antes de cerrar
        {
            MSG pm;
            while (PeekMessageW(&pm, h, datos::WM_APP_RED, datos::WM_APP_RED, PM_REMOVE)) {
                if (pm.wParam == datos::INFORME_PASADA) {
                    std::unique_ptr<datos::InformePasada> inf((datos::InformePasada*)pm.lParam);
                    pasada_hecha(*inf);
                } else if (pm.wParam == datos::INFORME_EXAMINAR) {
                    app.examen.reset((datos::InformeExaminar*)pm.lParam);
                }
            }
            atender_pendientes();
        }
        DestroyWindow(h);
        return 0;
    case WM_DESTROY:
        PostQuitMessage(0);
        return 0;
    }
    return DefWindowProcW(h, m, w, l);
}

}  // namespace

int WINAPI wWinMain(HINSTANCE inst, HINSTANCE, PWSTR, int show) {
    app.inst = inst;
    // Una sola ventana: escribe archivos compartidos (catalogo, trabajos de red) y
    // dos a la vez se pisarian. Si ya hay una, se le pasa el archivo y se sale.
    HANDLE unica = CreateMutexW(nullptr, TRUE, L"NestTubo-ventana-unica");
    if (unica && GetLastError() == ERROR_ALREADY_EXISTS) {
        HWND otra = FindWindowW(L"NestTuboVentana", nullptr);
        if (otra) {
            if (IsIconic(otra)) ShowWindow(otra, SW_RESTORE);
            SetForegroundWindow(otra);
            int ac = 0;
            LPWSTR* av = CommandLineToArgvW(GetCommandLineW(), &ac);
            if (av && ac > 1) {
                COPYDATASTRUCT cds{};
                cds.cbData = (DWORD)((wcslen(av[1]) + 1) * sizeof(wchar_t));
                cds.lpData = av[1];
                SendMessageW(otra, WM_COPYDATA, 0, (LPARAM)&cds);
            }
            if (av) LocalFree(av);
        }
        return 0;
    }
    OleInitialize(nullptr);   // el dialogo de carpeta nuevo lo necesita
    INITCOMMONCONTROLSEX ic{sizeof(ic), ICC_LISTVIEW_CLASSES | ICC_BAR_CLASSES | ICC_STANDARD_CLASSES};
    InitCommonControlsEx(&ic);
    WNDCLASSEXW wc{};
    wc.cbSize = sizeof(wc);
    wc.lpfnWndProc = Proc;
    wc.hInstance = inst;
    wc.hCursor = LoadCursorW(nullptr, IDC_ARROW);
    wc.hbrBackground = (HBRUSH)(COLOR_BTNFACE + 1);
    wc.lpszClassName = L"NestTuboVentana";
    wc.hIcon = LoadIconW(inst, MAKEINTRESOURCEW(1));
    wc.hIconSm = wc.hIcon;
    RegisterClassExW(&wc);
    HDC dc = GetDC(nullptr);
    int dpi = GetDeviceCaps(dc, LOGPIXELSX);
    ReleaseDC(nullptr, dc);
    HWND h = CreateWindowExW(0, L"NestTuboVentana", L"NestTubo", WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT,
                             MulDiv(1360, dpi, 96), MulDiv(820, dpi, 96), nullptr, nullptr, inst, nullptr);
    ShowWindow(h, show);
    UpdateWindow(h);
    int argc = 0;
    LPWSTR* argv = CommandLineToArgvW(GetCommandLineW(), &argc);
    if (argv && argc > 1) abrir(argv[1]);
    if (argv) LocalFree(argv);
    MSG msg;
    while (GetMessageW(&msg, nullptr, 0, 0) > 0) {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    return 0;
}
