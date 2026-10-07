// NestTubo - ventana Win32. Solo muestra y edita: el calculo vive en nucleo.cpp
// y la lectura del trabajo en trabajo.cpp.
#include <windows.h>
#include <commctrl.h>
#include <commdlg.h>
#include <shellapi.h>
#include <shlobj.h>
#include <shobjidl.h>

#include <algorithm>
#include <atomic>
#include <chrono>
#include <memory>
#include <string>
#include <thread>
#include <vector>

#include "nucleo.h"
#include "salidas.h"
#include "trabajo.h"

using namespace nt;

namespace {

// Tope de tiempo por perfil: al vencer se entrega lo mejor que haya.
constexpr int TOPE_SEGUNDOS = 20;

enum : int {
    ID_PERFILES = 101, ID_PIEZAS, ID_RESUMEN, ID_PLAN, ID_ESTADO,
    ID_NUEVO = 201, ID_ABRIR, ID_GUARDAR, ID_GUARDAR_COMO, ID_CALCULAR, ID_CANCELAR, ID_QUITAR_PIEZA, ID_QUITAR_PERFIL, ID_EXPORTAR,
    ID_ET_PERFILES = 301, ID_ET_PIEZAS, ID_ET_RESUMEN, ID_ET_PLAN,
};
enum : UINT { WM_APP_MOVER = WM_APP + 1, WM_APP_PROGRESO, WM_APP_FIN };
enum Movimiento { MOV_SIGUIENTE_COL = 1, MOV_ANTERIOR_COL, MOV_ABAJO, MOV_ARRIBA };

// ---------------------------------------------------------------------------
// Texto

std::wstring W(const std::string& s) {
    if (s.empty()) return L"";
    int n = MultiByteToWideChar(CP_UTF8, 0, s.data(), (int)s.size(), nullptr, 0);
    std::wstring r(n, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, s.data(), (int)s.size(), &r[0], n);
    return r;
}

std::string U(const std::wstring& s) {
    if (s.empty()) return "";
    int n = WideCharToMultiByte(CP_UTF8, 0, s.data(), (int)s.size(), nullptr, 0, nullptr, nullptr);
    std::string r(n, '\0');
    WideCharToMultiByte(CP_UTF8, 0, s.data(), (int)s.size(), &r[0], n, nullptr, nullptr);
    return r;
}

std::string minus(const std::string& s) {
    std::string r;
    size_t a = s.find_first_not_of(" \t"), b = s.find_last_not_of(" \t");
    if (a != std::string::npos) r = s.substr(a, b - a + 1);
    for (char& c : r)
        if (c >= 'A' && c <= 'Z') c = (char)(c - 'A' + 'a');
    return r;
}

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
} app;

int S(int v) { return MulDiv(v, app.dpi, 96); }

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
        if (cel && *cel != nuevo) {
            bool era_vacia = es_perfiles(lv) ? fila_vacia(app.trabajo.perfiles[f]) : fila_vacia(app.trabajo.piezas[f]);
            *cel = nuevo;
            poner_texto(lv, f, c, t);
            if (era_vacia && !nuevo.empty()) {
                if (es_perfiles(lv)) {
                    // perfil nuevo: valores iniciales en lo que no se haya escrito
                    PerfilTxt& p = app.trabajo.perfiles[f];
                    PerfilTxt ini = perfil_nuevo(p.nombre);
                    std::string* campos[] = {&p.barra, &p.despunte, &p.zona_muerta, &p.separacion, &p.margen};
                    std::string* valores[] = {&ini.barra, &ini.despunte, &ini.zona_muerta, &ini.separacion, &ini.margen};
                    for (int k = 0; k < 5; k++)
                        if (campos[k]->empty() && campos[k] != cel) *campos[k] = *valores[k];
                    for (int k = 1; k < 7; k++) poner_texto(lv, f, k, W(*celda(lv, f, k)));
                } else if (c != 0 && f > 0 && app.trabajo.piezas[f].perfil.empty()) {
                    // pieza nueva: mismo perfil que la fila de arriba
                    app.trabajo.piezas[f].perfil = app.trabajo.piezas[f - 1].perfil;
                    poner_texto(lv, f, 0, W(app.trabajo.piezas[f].perfil));
                }
            }
            if (!es_perfiles(lv) && c == 0) perfil_si_falta(nuevo);
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

// Si la pieza nombra un perfil que no esta en la tabla, se agrega con los valores iniciales.
void perfil_si_falta(const std::string& nombre) {
    if (minus(nombre).empty()) return;
    for (const auto& p : app.trabajo.perfiles)
        if (minus(p.nombre) == minus(nombre)) return;
    if (!app.trabajo.perfiles.empty() && fila_vacia(app.trabajo.perfiles.back())) {
        app.trabajo.perfiles.back() = perfil_nuevo(nombre);
        int f = (int)app.trabajo.perfiles.size() - 1;
        for (int k = 0; k < 7; k++) poner_texto(app.perfiles, f, k, W(*celda(app.perfiles, f, k)));
    } else {
        app.trabajo.perfiles.push_back(perfil_nuevo(nombre));
        agregar_fila_lv(app.perfiles);
    }
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
        MessageBoxW(app.wnd, msg.c_str(), L"NestTubo", MB_ICONWARNING);
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

std::wstring carpeta_config() {
    wchar_t ruta[MAX_PATH];
    if (FAILED(SHGetFolderPathW(nullptr, CSIDL_APPDATA, nullptr, 0, ruta))) return L"";
    std::wstring c = std::wstring(ruta) + L"\\NestTubo";
    CreateDirectoryW(c.c_str(), nullptr);
    return c;
}

std::wstring leer_config(const wchar_t* clave) {
    wchar_t v[MAX_PATH] = L"";
    GetPrivateProfileStringW(L"NestTubo", clave, L"", v, MAX_PATH, (carpeta_config() + L"\\config.ini").c_str());
    return v;
}

void escribir_config(const wchar_t* clave, const std::wstring& v) {
    WritePrivateProfileStringW(L"NestTubo", clave, v.c_str(), (carpeta_config() + L"\\config.ini").c_str());
}

bool leer_archivo(const std::wstring& ruta, std::string& datos) {
    HANDLE h = CreateFileW(ruta.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING, 0, nullptr);
    if (h == INVALID_HANDLE_VALUE) return false;
    LARGE_INTEGER t;
    bool ok = GetFileSizeEx(h, &t) && t.QuadPart < (64 << 20);
    if (ok) {
        datos.resize((size_t)t.QuadPart);
        DWORD leidos = 0;
        ok = datos.empty() || (ReadFile(h, &datos[0], (DWORD)datos.size(), &leidos, nullptr) && leidos == datos.size());
    }
    CloseHandle(h);
    return ok;
}

// Escribe en un temporal y lo pone en su sitio de una vez: un corte no deja el archivo a medias.
bool escribir_archivo(const std::wstring& ruta, const std::string& datos) {
    std::wstring tmp = ruta + L".tmp";
    HANDLE h = CreateFileW(tmp.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) return false;
    DWORD escritos = 0;
    bool ok = WriteFile(h, datos.data(), (DWORD)datos.size(), &escritos, nullptr) && escritos == datos.size();
    ok = FlushFileBuffers(h) && ok;
    CloseHandle(h);
    if (ok) ok = MoveFileExW(tmp.c_str(), ruta.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH);
    if (!ok) DeleteFileW(tmp.c_str());
    return ok;
}

// El catalogo de perfiles sobrevive entre trabajos: se guarda al cerrar y se carga al abrir.
void guardar_catalogo() {
    Trabajo cat;
    cat.perfiles = app.trabajo.perfiles;
    escribir_archivo(carpeta_config() + L"\\perfiles.ntb", a_texto(cat));
}

void cargar_catalogo() {
    std::string datos, error;
    Trabajo cat;
    if (leer_archivo(carpeta_config() + L"\\perfiles.ntb", datos) && de_texto(datos, cat, error)) app.trabajo.perfiles = cat.perfiles;
}

bool dialogo_archivo(bool guardar, std::wstring& ruta) {
    wchar_t buf[MAX_PATH] = L"";
    if (!ruta.empty()) lstrcpynW(buf, ruta.c_str(), MAX_PATH);
    std::wstring inicial = leer_config(L"carpeta");
    OPENFILENAMEW o{};
    o.lStructSize = sizeof(o);
    o.hwndOwner = app.wnd;
    o.lpstrFilter = L"Trabajos de NestTubo (*.ntb)\0*.ntb\0Todos los archivos\0*.*\0";
    o.lpstrFile = buf;
    o.nMaxFile = MAX_PATH;
    o.lpstrDefExt = L"ntb";
    o.lpstrInitialDir = inicial.empty() ? nullptr : inicial.c_str();
    o.Flags = guardar ? (OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST) : (OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST);
    if (!(guardar ? GetSaveFileNameW(&o) : GetOpenFileNameW(&o))) return false;
    ruta = buf;
    size_t k = ruta.find_last_of(L"\\/");
    if (k != std::wstring::npos) escribir_config(L"carpeta", ruta.substr(0, k));
    return true;
}

bool guardar(bool como) {
    cerrar_edicion(false);
    std::wstring ruta = app.archivo;
    if (como || ruta.empty())
        if (!dialogo_archivo(true, ruta)) return false;
    if (!escribir_archivo(ruta, a_texto(app.trabajo))) {
        MessageBoxW(app.wnd, (L"No se pudo guardar en\n" + ruta).c_str(), L"NestTubo", MB_ICONERROR);
        return false;
    }
    app.archivo = ruta;
    app.modificado = false;
    titulo();
    guardar_catalogo();
    estado(L"Guardado en " + ruta);
    return true;
}

// true si se puede seguir (no habia cambios, se guardaron o se descartaron)
bool puede_descartar() {
    cerrar_edicion(false);
    if (!app.modificado) return true;
    int r = MessageBoxW(app.wnd, L"¿Guardar los cambios del trabajo actual?", L"NestTubo", MB_YESNOCANCEL | MB_ICONQUESTION);
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
bool carpeta_moderna(const std::wstring& inicial, std::wstring& carpeta, bool& elegida) {
    IFileOpenDialog* d = nullptr;
    if (FAILED(CoCreateInstance(CLSID_FileOpenDialog, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&d)))) return false;
    DWORD op = 0;
    d->GetOptions(&op);
    d->SetOptions(op | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST);
    d->SetTitle(L"Carpeta donde dejar el PDF, el Excel y los DXF");
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

bool elegir_carpeta(std::wstring& carpeta) {
    std::wstring inicial = leer_config(L"carpeta_exportar");
    if (inicial.empty()) inicial = leer_config(L"carpeta");
    bool elegida = false;
    if (carpeta_moderna(inicial, carpeta, elegida)) {
        if (elegida) escribir_config(L"carpeta_exportar", carpeta);
        return elegida;
    }
    BROWSEINFOW b{};
    b.hwndOwner = app.wnd;
    b.lpszTitle = L"Carpeta donde dejar el PDF, el Excel y los DXF:";
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
    escribir_config(L"carpeta_exportar", carpeta);
    return true;
}

bool existe(const std::wstring& ruta) { return GetFileAttributesW(ruta.c_str()) != INVALID_FILE_ATTRIBUTES; }

void exportar() {
    cerrar_edicion(false);
    if (app.calculando) return;
    if (app.resultados.empty()) {
        MessageBoxW(app.wnd, L"Primero calcula las barras: se exporta el último cálculo.", L"NestTubo", MB_ICONINFORMATION);
        return;
    }
    if (app.resultado_viejo) {
        MessageBoxW(app.wnd, L"Los datos cambiaron desde el último cálculo. Calcula de nuevo antes de exportar.", L"NestTubo",
                    MB_ICONWARNING);
        return;
    }
    std::wstring carpeta;
    if (!elegir_carpeta(carpeta)) return;
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
        if (MessageBoxW(app.wnd, q.c_str(), L"NestTubo", MB_YESNO | MB_ICONQUESTION) != IDYES) return;
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
    if (!escribir_archivo(pdf, informe_pdf(app.resultados, trabajo, fecha))) falla = pdf;
    else if (!escribir_archivo(xlsx, informe_xlsx(app.resultados, trabajo, fecha))) falla = xlsx;
    int n_dxf = 0;
    if (falla.empty()) {
        CreateDirectoryW(dxf.c_str(), nullptr);
        for (const auto& r : app.resultados) {
            for (const auto& d : dxf_perfil(r)) {
                std::wstring ruta = dxf + L"\\" + W(d.nombre);
                if (!escribir_archivo(ruta, d.texto)) { falla = ruta; break; }
                n_dxf++;
            }
            if (!falla.empty()) break;
        }
    }
    if (!falla.empty()) {
        estado(L"No se pudo exportar.");
        MessageBoxW(app.wnd, (L"No se pudo escribir\n" + falla + L"\n\n¿Está abierto en otro programa? Ciérralo y vuelve a exportar.").c_str(),
                    L"NestTubo", MB_ICONERROR);
        return;
    }
    estado(L"Exportado en " + carpeta + L": PDF, Excel y " + std::to_wstring(n_dxf) + L" DXF.");
    std::wstring msg = L"Listo. En " + carpeta + L" quedaron:\n\n" + nombre + L" - plan.pdf\n" + nombre + L" - plan.xlsx\n" +
                       nombre + L" - DXF  (" + std::to_wstring(n_dxf) + (n_dxf == 1 ? L" archivo)" : L" archivos)") +
                       L"\n\n¿Abrir la carpeta?";
    if (MessageBoxW(app.wnd, msg.c_str(), L"NestTubo", MB_YESNO | MB_ICONINFORMATION) == IDYES)
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
    app.trabajo.piezas.clear();   // los perfiles se quedan: son el catalogo
    asegurar_fila_vacia();
    app.archivo.clear();
    app.modificado = false;
    llenar_tabla(app.perfiles);
    llenar_tabla(app.piezas);
    limpiar_resultado();
    titulo();
    estado(L"Trabajo nuevo. Los perfiles se conservan.");
}

// Sin ruta pregunta con el dialogo; con ruta (la que llega al arrastrar un
// .ntb sobre el programa o al usar "Abrir con") la abre directo.
void abrir(std::wstring ruta = L"") {
    if (!puede_descartar()) return;
    if (ruta.empty() && !dialogo_archivo(false, ruta)) return;
    std::string datos, error;
    Trabajo t;
    if (!leer_archivo(ruta, datos) || !de_texto(datos, t, error)) {
        MessageBoxW(app.wnd, (L"No se pudo abrir " + ruta + L"\n" + W(error)).c_str(), L"NestTubo", MB_ICONERROR);
        return;
    }
    app.trabajo = t;
    asegurar_fila_vacia();
    app.archivo = ruta;
    app.modificado = false;
    llenar_tabla(app.perfiles);
    llenar_tabla(app.piezas);
    limpiar_resultado();
    titulo();
    estado(L"Abierto " + ruta);
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
    std::wstring q = L"¿Quitar " + std::to_wstring(sel.size()) + (sel.size() == 1 ? L" fila" : L" filas") +
                     (es_perfiles(lv) ? L" de perfiles?" : L" de piezas?");
    if (MessageBoxW(app.wnd, q.c_str(), L"NestTubo", MB_YESNO | MB_ICONQUESTION) != IDYES) return;
    for (size_t k = sel.size(); k-- > 0;) {
        if (es_perfiles(lv)) app.trabajo.perfiles.erase(app.trabajo.perfiles.begin() + sel[k]);
        else app.trabajo.piezas.erase(app.trabajo.piezas.begin() + sel[k]);
    }
    asegurar_fila_vacia();
    llenar_tabla(lv);
    datos_cambiaron();
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
        cargar_catalogo();
        asegurar_fila_vacia();
        llenar_tabla(app.perfiles);
        llenar_tabla(app.piezas);
        titulo();
        estado(L"Escribe los perfiles y las piezas del pedido y pulsa \"Calcular barras\". Clic en una celda para editarla.");
        return 0;
    }
    case WM_SIZE:
        acomodar();
        return 0;
    case WM_GETMINMAXINFO: {
        auto* mm = (MINMAXINFO*)l;
        mm->ptMinTrackSize.x = S(950);
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
    case WM_COMMAND:
        switch (LOWORD(w)) {
        case ID_NUEVO: nuevo(); break;
        case ID_ABRIR: abrir(); break;
        case ID_GUARDAR: guardar(false); break;
        case ID_GUARDAR_COMO: guardar(true); break;
        case ID_CALCULAR: calcular(); break;
        case ID_EXPORTAR: exportar(); break;
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
    case WM_CLOSE:
        if (app.calculando) {
            app.cancelar = true;
            if (app.hilo.joinable()) app.hilo.join();
            app.calculando = false;
        }
        if (!puede_descartar()) return 0;
        guardar_catalogo();
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
