// NestTubo - carpeta de datos (ver datos.h).
#include "datos.h"

#include <shlobj.h>

#include <algorithm>
#include <chrono>
#include <condition_variable>
#include <map>
#include <mutex>
#include <thread>

using namespace nt;

namespace datos {

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

namespace {

// Las rutas propias se calculan una vez. Se guardan en memoria que nunca se
// libera: el hilo de red sigue vivo mientras el proceso termina.
std::wstring carpeta_especial(int csidl) {
    wchar_t ruta[MAX_PATH];
    if (FAILED(SHGetFolderPathW(nullptr, csidl, nullptr, 0, ruta))) return L"";
    std::wstring c = std::wstring(ruta) + L"\\NestTubo";
    CreateDirectoryW(c.c_str(), nullptr);
    return c;
}

const std::wstring& config_() {
    static const std::wstring* c = new std::wstring(carpeta_especial(CSIDL_APPDATA));
    return *c;
}

const std::wstring& local_() {
    static const std::wstring* c = new std::wstring([] {
        std::wstring l = carpeta_especial(CSIDL_LOCAL_APPDATA);
        return l.empty() ? carpeta_especial(CSIDL_APPDATA) : l;
    }());
    return *c;
}

std::wstring ruta_copia() { return local_() + L"\\perfiles.ntb"; }
std::wstring ruta_pendientes() { return local_() + L"\\perfiles_pendientes.ntb"; }

// Mutex con nombre "NestTubo-local": solo protege archivos de este equipo y
// nunca se tiene durante una operacion de red.
class Bloqueo {
    HANDLE h_ = nullptr;
    bool tengo_ = false;

public:
    Bloqueo() {
        h_ = CreateMutexW(nullptr, FALSE, L"NestTubo-local");
        if (h_) {
            DWORD r = WaitForSingleObject(h_, 10000);
            tengo_ = r == WAIT_OBJECT_0 || r == WAIT_ABANDONED;
        }
    }
    ~Bloqueo() {
        if (tengo_) ReleaseMutex(h_);
        if (h_) CloseHandle(h_);
    }
    Bloqueo(const Bloqueo&) = delete;
    Bloqueo& operator=(const Bloqueo&) = delete;
};

std::wstring con_barra(const std::wstring& c) { return !c.empty() && c.back() == L'\\' ? c : c + L'\\'; }

std::wstring sin_barra(std::wstring c) {
    while (c.size() > 3 && c.back() == L'\\') c.pop_back();
    return c;
}

std::wstring normal(std::wstring r) {
    std::replace(r.begin(), r.end(), L'/', L'\\');
    return r;
}

std::string texto_catalogo(const std::vector<PerfilTxt>& perfiles) {
    Trabajo t;
    t.perfiles = perfiles;
    return a_texto(t);
}

bool leer_catalogo(const std::wstring& ruta, std::vector<PerfilTxt>& perfiles, Lectura& l, std::wstring& motivo) {
    std::string d, e;
    DWORD cod = 0;
    l = leer_archivo(ruta, d, &cod);
    if (l == Lectura::error) {
        motivo = texto_error(cod);
        return false;
    }
    if (l == Lectura::no_existe) return false;
    Trabajo t;
    if (!de_texto(d, t, e)) {
        l = Lectura::error;
        motivo = L"no es un catálogo de NestTubo (" + W(e) + L")";
        return false;
    }
    perfiles = aplicar_cambios(t.perfiles, CambiosCatalogo{});
    return true;
}

}  // namespace

// ---------------------------------------------------------------------------

std::wstring carpeta_config() { return config_(); }
std::wstring carpeta_local() { return local_(); }
std::wstring carpeta_espera() { return local_() + L"\\Sin conexión"; }

std::wstring leer_config(const wchar_t* clave) {
    wchar_t v[1024] = L"";
    GetPrivateProfileStringW(L"NestTubo", clave, L"", v, 1024, (config_() + L"\\config.ini").c_str());
    return v;
}

void escribir_config(const wchar_t* clave, const std::wstring& valor) {
    WritePrivateProfileStringW(L"NestTubo", clave, valor.c_str(), (config_() + L"\\config.ini").c_str());
}

std::wstring carpeta_documentos() {
    wchar_t ruta[MAX_PATH];
    if (FAILED(SHGetFolderPathW(nullptr, CSIDL_PERSONAL, nullptr, 0, ruta))) return L"";
    return ruta;
}

// ---------------------------------------------------------------------------
// Archivos

Lectura leer_archivo(const std::wstring& ruta, std::string& datos, DWORD* codigo) {
    datos.clear();
    if (codigo) *codigo = 0;
    HANDLE h = CreateFileW(ruta.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
                           OPEN_EXISTING, 0, nullptr);
    if (h == INVALID_HANDLE_VALUE) {
        DWORD e = GetLastError();
        if (codigo) *codigo = e;
        return e == ERROR_FILE_NOT_FOUND ? Lectura::no_existe : Lectura::error;
    }
    LARGE_INTEGER t;
    DWORD e = 0;
    bool ok = GetFileSizeEx(h, &t);
    if (!ok) e = GetLastError();
    else if (t.QuadPart >= (64 << 20)) {
        ok = false;
        e = ERROR_FILE_TOO_LARGE;
    }
    if (ok) {
        datos.resize((size_t)t.QuadPart);
        DWORD leidos = 0;
        if (!datos.empty()) {
            ok = ReadFile(h, &datos[0], (DWORD)datos.size(), &leidos, nullptr);
            if (!ok) e = GetLastError();
            else if (leidos != datos.size()) {
                ok = false;
                e = ERROR_HANDLE_EOF;
            }
        }
    }
    CloseHandle(h);
    if (!ok) {
        datos.clear();
        if (codigo) *codigo = e;
        return Lectura::error;
    }
    return Lectura::ok;
}

bool escribir_archivo(const std::wstring& ruta, const std::string& datos, DWORD* codigo) {
    std::wstring tmp = ruta + L"." + std::to_wstring(GetCurrentProcessId()) + L".tmp";
    DWORD e = 0;
    for (int intento = 0; intento < 4; intento++) {
        if (intento) Sleep(200);
        HANDLE h = CreateFileW(tmp.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
        if (h == INVALID_HANDLE_VALUE) {
            e = GetLastError();
        } else {
            DWORD escritos = 0;
            bool ok = WriteFile(h, datos.data(), (DWORD)datos.size(), &escritos, nullptr) && escritos == datos.size();
            if (!ok) e = GetLastError();
            if (ok && !FlushFileBuffers(h)) {
                ok = false;
                e = GetLastError();
            }
            CloseHandle(h);
            if (ok && MoveFileExW(tmp.c_str(), ruta.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)) {
                if (codigo) *codigo = 0;
                return true;
            }
            if (ok) e = GetLastError();
            DeleteFileW(tmp.c_str());
        }
        if (e != ERROR_SHARING_VIOLATION && e != ERROR_ACCESS_DENIED && e != ERROR_LOCK_VIOLATION) break;
    }
    if (codigo) *codigo = e ? e : ERROR_WRITE_FAULT;
    return false;
}

bool crear_carpetas(const std::wstring& carpeta) {
    std::wstring c = sin_barra(normal(carpeta));
    if (c.empty()) return false;
    DWORD a = GetFileAttributesW(c.c_str());
    if (a != INVALID_FILE_ATTRIBUTES) return (a & FILE_ATTRIBUTE_DIRECTORY) != 0;
    if (clave_ruta(c) == clave_ruta(raiz(c))) return false;   // la raiz no responde
    std::wstring padre = carpeta_de(c);
    if (padre.empty() || padre == c) return false;
    if (!crear_carpetas(padre)) return false;
    return CreateDirectoryW(c.c_str(), nullptr) || GetLastError() == ERROR_ALREADY_EXISTS;
}

bool existe(const std::wstring& ruta) { return GetFileAttributesW(ruta.c_str()) != INVALID_FILE_ATTRIBUTES; }

std::wstring carpeta_de(const std::wstring& ruta) {
    std::wstring r = normal(ruta);
    size_t k = r.find_last_of(L'\\');
    if (k == std::wstring::npos) return L"";
    if (k == 2 && r.size() > 1 && r[1] == L':') return r.substr(0, 3);   // "C:\"
    return r.substr(0, k);
}

std::wstring nombre_de(const std::wstring& ruta) {
    std::wstring r = normal(ruta);
    size_t k = r.find_last_of(L'\\');
    return k == std::wstring::npos ? r : r.substr(k + 1);
}

std::wstring texto_error(DWORD codigo) {
    wchar_t* buf = nullptr;
    DWORD n = FormatMessageW(FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                             nullptr, codigo, 0, (LPWSTR)&buf, 0, nullptr);
    std::wstring t = n && buf ? std::wstring(buf, n) : L"";
    if (buf) LocalFree(buf);
    while (!t.empty() && (t.back() == L'\r' || t.back() == L'\n' || t.back() == L' ' || t.back() == L'.')) t.pop_back();
    if (t.empty()) t = L"error";
    return t + L" (código " + std::to_wstring(codigo) + L")";
}

std::wstring raiz(const std::wstring& ruta) {
    std::wstring r = normal(ruta);
    if (r.size() > 2 && r[0] == L'\\' && r[1] == L'\\') {
        if (r[2] == L'?' || r[2] == L'.' || r[2] == L'\\') return L"";
        size_t a = r.find(L'\\', 2);
        if (a == std::wstring::npos || a == 2 || a + 1 >= r.size() || r[a + 1] == L'\\') return L"";
        size_t b = r.find(L'\\', a + 1);
        return b == std::wstring::npos ? r : r.substr(0, b);
    }
    if (r.size() >= 2 && iswalpha(r[0]) && r[1] == L':' && (r.size() == 2 || r[2] == L'\\'))
        return std::wstring(1, (wchar_t)towupper(r[0])) + L":\\";
    return L"";
}

bool es_remota(const std::wstring& ruta) {
    std::wstring r = raiz(ruta);
    if (r.empty()) return false;
    if (r[0] == L'\\') return true;
    // Una letra que hoy no existe (unidad de red quitada, disco sacado) se trata
    // igual: lo guardado espera en este equipo hasta que la letra vuelva.
    UINT t = GetDriveTypeW(r.c_str());
    return t == DRIVE_REMOTE || t == DRIVE_NO_ROOT_DIR;
}

std::wstring clave_ruta(const std::wstring& ruta) {
    std::wstring r = sin_barra(normal(ruta));
    if (!r.empty()) CharLowerBuffW(&r[0], (DWORD)r.size());
    return r;
}

// ---------------------------------------------------------------------------
// Trabajos en espera

std::wstring ruta_en_espera(const std::wstring& destino) {
    return W(ruta_pendiente(U(carpeta_espera()), U(normal(destino))));
}

bool destino_en_espera(const std::wstring& ruta, std::wstring& destino) {
    std::string d;
    if (!destino_de_pendiente(U(carpeta_espera()), U(normal(ruta)), d)) return false;
    destino = W(d);
    return true;
}

std::wstring carpeta_en_espera(const std::wstring& carpeta) {
    std::wstring r = ruta_en_espera(sin_barra(normal(carpeta)) + L"\\x");
    return r.size() > 2 ? r.substr(0, r.size() - 2) : L"";
}

bool guardar_trabajo(const std::wstring& destino, const std::string& texto, bool& en_espera, std::wstring& error) {
    en_espera = false;
    DWORD cod = 0;
    if (es_remota(destino)) {
        std::wstring p = ruta_en_espera(destino);
        if (p.empty()) {
            error = L"No se puede guardar en\n" + destino;
            return false;
        }
        if (p.size() >= MAX_PATH - 16) {
            error = L"La copia en este equipo tendría una ruta demasiado larga:\n" + p;
            return false;
        }
        Bloqueo b;
        crear_carpetas(carpeta_de(p));
        if (!escribir_archivo(p, texto, &cod)) {
            error = L"No se pudo guardar en este equipo:\n" + p + L"\n\n" + texto_error(cod);
            return false;
        }
        en_espera = true;
        return true;
    }
    if (!escribir_archivo(destino, texto, &cod)) {
        error = L"No se pudo guardar en\n" + destino + L"\n\n" + texto_error(cod);
        return false;
    }
    return true;
}

std::wstring version_a_abrir(const std::wstring& ruta, std::wstring& destino, bool& en_espera) {
    en_espera = false;
    destino = ruta;
    std::wstring d;
    if (destino_en_espera(ruta, d)) {
        destino = d;
        en_espera = true;
        return ruta;
    }
    if (es_remota(ruta)) {
        std::wstring p = ruta_en_espera(ruta);
        if (!p.empty() && existe(p)) {
            en_espera = true;
            return p;
        }
    }
    return ruta;
}

// ---------------------------------------------------------------------------
// Catalogo

bool cargar_catalogo(std::vector<PerfilTxt>& catalogo, std::wstring& error) {
    Bloqueo b;
    std::wstring copia = ruta_copia();
    if (!existe(copia)) {   // migracion desde 0.2: el catalogo vivia en %APPDATA%
        std::wstring vieja = config_() + L"\\perfiles.ntb";
        if (clave_ruta(vieja) != clave_ruta(copia) && existe(vieja)) CopyFileW(vieja.c_str(), copia.c_str(), TRUE);
    }
    catalogo.clear();
    Lectura l;
    std::wstring motivo;
    bool ok = leer_catalogo(copia, catalogo, l, motivo) || l == Lectura::no_existe;
    if (!ok) error = L"No se pudo leer " + copia + L": " + motivo;
    if (!leer_config(L"carpeta_datos").empty()) {
        std::string d, e;
        CambiosCatalogo p;
        if (leer_archivo(ruta_pendientes(), d) == Lectura::ok && de_texto_cambios(d, p, e)) catalogo = aplicar_cambios(catalogo, p);
    }
    return ok;
}

std::vector<PerfilTxt> copia_local(const std::vector<PerfilTxt>& si_falla) {
    Bloqueo b;
    std::vector<PerfilTxt> c;
    Lectura l;
    std::wstring motivo;
    return leer_catalogo(ruta_copia(), c, l, motivo) ? c : si_falla;
}

bool registrar(const CambiosCatalogo& c, std::vector<PerfilTxt>& catalogo, std::wstring& error) {
    if (c.vacio()) return true;
    Bloqueo b;
    // partir de la copia en disco (otra ventana pudo cambiarla); si no se puede leer, de la memoria
    std::vector<PerfilTxt> base;
    Lectura l;
    std::wstring motivo;
    if (!leer_catalogo(ruta_copia(), base, l, motivo)) base = catalogo;
    std::vector<PerfilTxt> resultado = aplicar_cambios(base, c);
    bool ok = true;
    DWORD cod = 0;
    if (!leer_config(L"carpeta_datos").empty()) {
        // primero los pendientes: si se corta antes de la copia local, al arrancar se aplican
        std::string d, e;
        CambiosCatalogo pend;
        Lectura lp = leer_archivo(ruta_pendientes(), d);
        if (lp == Lectura::error || (lp == Lectura::ok && !de_texto_cambios(d, pend, e))) {
            pend = CambiosCatalogo{};   // ilegibles: se manda todo el catalogo
            pend.cambiados = base;
        }
        juntar_cambios(pend, c);
        if (!escribir_archivo(ruta_pendientes(), a_texto_cambios(pend), &cod)) {
            ok = false;
            error = L"No se pudieron anotar los cambios de perfiles en " + ruta_pendientes() + L": " + texto_error(cod);
        }
    }
    if (!escribir_archivo(ruta_copia(), texto_catalogo(resultado), &cod)) {
        ok = false;
        error = L"No se pudo guardar el catálogo de perfiles en " + ruta_copia() + L": " + texto_error(cod);
    }
    catalogo = resultado;
    return ok;
}

bool cambiar_carpeta(const std::wstring& carpeta, const std::vector<PerfilTxt>& resultado, bool respaldar, std::wstring& error) {
    Bloqueo b;
    std::wstring copia = ruta_copia();
    DWORD cod = 0;
    if (respaldar && existe(copia)) {
        std::wstring resp = local_() + L"\\perfiles antes de cambiar de carpeta.ntb";
        if (!CopyFileW(copia.c_str(), resp.c_str(), FALSE)) {
            error = L"No se pudo respaldar el catálogo de este equipo en " + resp + L": " + texto_error(GetLastError());
            return false;
        }
    }
    CambiosCatalogo pend;
    pend.cambiados = resultado;
    if (!escribir_archivo(ruta_pendientes(), a_texto_cambios(pend), &cod) ||
        !escribir_archivo(copia, texto_catalogo(resultado), &cod)) {
        error = L"No se pudo guardar el catálogo de perfiles en este equipo: " + texto_error(cod);
        return false;
    }
    std::wstring c = sin_barra(carpeta);
    escribir_config(L"carpeta_datos", c);
    escribir_config(L"carpeta", c);
    escribir_config(L"carpeta_exportar", c);
    return true;
}

// ---------------------------------------------------------------------------
// Hilo de red

namespace {

struct Hilo {
    std::mutex m;
    std::condition_variable cv, cv_fin;
    bool pasada = false;
    std::vector<std::wstring> examinar;
    std::wstring archivo;
    unsigned long long pedidas = 0, terminadas = 0;
    HWND ventana = nullptr;
};

Hilo& hilo() {
    static Hilo* h = new Hilo;   // nunca se libera: el hilo vive hasta que termina el proceso
    return *h;
}

// Solo para pruebas: NESTTUBO_DEMORA_RED_MS simula una red lenta.
DWORD demora_red() {
    static const DWORD d = [] {
        wchar_t v[32] = L"";
        DWORD n = GetEnvironmentVariableW(L"NESTTUBO_DEMORA_RED_MS", v, 32);
        return n && n < 32 ? (DWORD)wcstoul(v, nullptr, 10) : (DWORD)0;
    }();
    return d;
}

void esperar_red() {
    if (demora_red()) Sleep(demora_red());
}

bool responde(const std::wstring& r) {
    if (r.empty()) return false;
    esperar_red();
    DWORD a = GetFileAttributesW(con_barra(r).c_str());
    return a != INVALID_FILE_ATTRIBUTES && (a & FILE_ATTRIBUTE_DIRECTORY);
}

template <class T>
void enviar(WPARAM tipo, T* informe) {
    if (!PostMessageW(hilo().ventana, WM_APP_RED, tipo, (LPARAM)informe)) delete informe;
}

void listar(const std::wstring& carpeta, std::vector<std::wstring>& archivos) {
    WIN32_FIND_DATAW f;
    HANDLE h = FindFirstFileW((carpeta + L"\\*").c_str(), &f);
    if (h == INVALID_HANDLE_VALUE) return;
    do {
        std::wstring n = f.cFileName;
        if (n == L"." || n == L"..") continue;
        std::wstring r = carpeta + L"\\" + n;
        if (f.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) listar(r, archivos);
        else if (!(n.size() > 4 && clave_ruta(n.substr(n.size() - 4)) == L".tmp")) archivos.push_back(r);
    } while (FindNextFileW(h, &f));
    FindClose(h);
}

void borrar_carpetas_vacias(std::wstring c) {
    std::wstring tope = clave_ruta(carpeta_espera());
    while (clave_ruta(c).size() > tope.size() && clave_ruta(c).compare(0, tope.size(), tope) == 0) {
        if (!RemoveDirectoryW(c.c_str())) break;
        c = carpeta_de(c);
    }
}

void sincronizar_catalogo(const std::wstring& carpeta, InformePasada& inf, std::map<std::wstring, bool>& en_linea) {
    std::string pend_bytes, e;
    CambiosCatalogo pend;
    std::vector<PerfilTxt> local;
    Lectura lp, ll;
    {
        Bloqueo b;
        lp = leer_archivo(ruta_pendientes(), pend_bytes);
        std::wstring motivo;
        leer_catalogo(ruta_copia(), local, ll, motivo);
        if (lp == Lectura::error) {
            inf.error_catalogo = L"No se pudieron leer los cambios de perfiles de este equipo (" + ruta_pendientes() + L")";
            inf.catalogo_en_espera = true;
            return;
        }
        if (lp == Lectura::ok && !de_texto_cambios(pend_bytes, pend, e)) {
            pend = CambiosCatalogo{};
            pend.cambiados = local;
        }
    }
    std::wstring archivo = carpeta + L"\\perfiles.ntb", motivo;
    std::vector<PerfilTxt> compartido;
    esperar_red();
    Lectura lc;
    leer_catalogo(archivo, compartido, lc, motivo);
    if (lc == Lectura::no_existe && ll == Lectura::error) {   // nunca crear el compartido desde una copia ilegible
        lc = Lectura::error;
        motivo = L"la copia de este equipo no se pudo leer";
    }
    PasadaCatalogo p = pasada_catalogo(lc, compartido, local, pend);
    if (lc == Lectura::error) {
        if (!responde(raiz(carpeta))) en_linea[clave_ruta(raiz(carpeta))] = false;
        else inf.error_catalogo = L"No se pudo leer " + archivo + L": " + motivo;
        inf.catalogo_en_espera = lp == Lectura::ok;
        return;
    }
    if (p.escribir) {
        esperar_red();
        DWORD cod = 0;
        if (!escribir_archivo(archivo, texto_catalogo(p.resultado), &cod)) {
            if (!responde(raiz(carpeta))) en_linea[clave_ruta(raiz(carpeta))] = false;
            else inf.error_catalogo = L"No se pudo escribir " + archivo + L": " + texto_error(cod);
            inf.catalogo_en_espera = true;
            return;
        }
    }
    Bloqueo b;
    if (clave_ruta(leer_config(L"carpeta_datos")) != clave_ruta(carpeta)) {   // cambiaron de carpeta mientras tanto
        inf.catalogo_en_espera = true;
        return;
    }
    std::string ahora;
    CambiosCatalogo quedan;
    Lectura l2 = leer_archivo(ruta_pendientes(), ahora);
    if (lp == Lectura::ok && l2 == Lectura::ok && ahora == pend_bytes) DeleteFileW(ruta_pendientes().c_str());
    else if (l2 == Lectura::ok && !de_texto_cambios(ahora, quedan, e)) quedan = CambiosCatalogo{};
    std::vector<PerfilTxt> nuevo = aplicar_cambios(p.resultado, quedan);
    std::string texto = texto_catalogo(nuevo), actual;
    if (leer_archivo(ruta_copia(), actual) != Lectura::ok || actual != texto) escribir_archivo(ruta_copia(), texto);
    inf.hay_catalogo = true;
    inf.catalogo = nuevo;
    inf.catalogo_en_espera = existe(ruta_pendientes());
}

void hacer_pasada(const std::wstring& archivo, InformePasada& inf) {
    std::vector<std::wstring> espera;
    listar(carpeta_espera(), espera);
    std::wstring carpeta = leer_config(L"carpeta_datos");
    inf.carpeta_datos = carpeta;
    HANDLE sinc = CreateMutexW(nullptr, FALSE, L"NestTubo-sincronizar");
    if (sinc) {
        DWORD r = WaitForSingleObject(sinc, 0);
        if (r != WAIT_OBJECT_0 && r != WAIT_ABANDONED) {
            CloseHandle(sinc);
            inf.saltada = true;
            inf.trabajos_en_espera = (int)espera.size();
            inf.catalogo_en_espera = existe(ruta_pendientes());
            return;
        }
    }
    // 1. que raices responden
    std::vector<std::wstring> rutas = {carpeta, leer_config(L"carpeta"), leer_config(L"carpeta_exportar"), archivo};
    std::vector<std::wstring> destinos(espera.size());
    for (size_t i = 0; i < espera.size(); i++)
        if (destino_en_espera(espera[i], destinos[i])) rutas.push_back(destinos[i]);
    std::map<std::wstring, bool> en_linea;
    std::map<std::wstring, std::wstring> nombre;
    for (const auto& r : rutas) {
        std::wstring R = raiz(r), k = clave_ruta(R);
        if (R.empty() || en_linea.count(k)) continue;
        nombre[k] = R;
        en_linea[k] = responde(R);
    }
    // 2. catalogo
    if (!carpeta.empty() && en_linea[clave_ruta(raiz(carpeta))]) sincronizar_catalogo(carpeta, inf, en_linea);
    else inf.catalogo_en_espera = existe(ruta_pendientes());
    // 3. trabajos
    for (size_t i = 0; i < espera.size(); i++) {
        const std::wstring& f = espera[i];
        const std::wstring& destino = destinos[i];
        std::wstring k = clave_ruta(raiz(destino));
        if (destino.empty() || !en_linea[k]) {
            inf.trabajos_en_espera++;
            continue;
        }
        std::string bytes;
        if (leer_archivo(f, bytes) != Lectura::ok) {
            inf.trabajos_en_espera++;
            continue;
        }
        esperar_red();
        crear_carpetas(carpeta_de(destino));
        DWORD cod = 0;
        if (escribir_archivo(destino, bytes, &cod)) {
            Bloqueo b;
            std::string ahora;
            if (leer_archivo(f, ahora) == Lectura::ok && ahora == bytes) {   // si se guardo otra vez, va en la siguiente
                DeleteFileW(f.c_str());
                borrar_carpetas_vacias(carpeta_de(f));
            } else {
                inf.trabajos_en_espera++;
            }
            inf.copiados.push_back(destino);
        } else {
            if (!responde(raiz(destino))) en_linea[k] = false;
            else inf.fallas.push_back(Falla{f, destino, cod});
            inf.trabajos_en_espera++;
        }
    }
    for (const auto& x : en_linea)
        if (!x.first.empty()) inf.raices.push_back({nombre[x.first], x.second ? Conexion::en_linea : Conexion::sin_conexion});
    if (sinc) {
        ReleaseMutex(sinc);
        CloseHandle(sinc);
    }
}

void hacer_examinar(const std::wstring& carpeta, InformeExaminar& inf) {
    inf.carpeta = carpeta;
    esperar_red();
    DWORD a = GetFileAttributesW(carpeta.c_str());
    if (a == INVALID_FILE_ATTRIBUTES || !(a & FILE_ATTRIBUTE_DIRECTORY)) {
        inf.lectura = Lectura::error;
        inf.error = L"la carpeta no responde: " + texto_error(GetLastError());
        return;
    }
    esperar_red();
    leer_catalogo(sin_barra(carpeta) + L"\\perfiles.ntb", inf.catalogo, inf.lectura, inf.error);
}

void bucle() {
    Hilo& h = hilo();
    for (;;) {
        std::wstring ex, archivo;
        unsigned long long numero = 0;
        {
            std::unique_lock<std::mutex> lk(h.m);
            if (!h.pasada && h.examinar.empty()) h.cv.wait_for(lk, std::chrono::seconds(30));
            if (!h.examinar.empty()) {
                ex = h.examinar.front();
                h.examinar.erase(h.examinar.begin());
            } else {
                h.pasada = false;
                numero = h.pedidas;
                archivo = h.archivo;
            }
        }
        if (!ex.empty()) {
            auto* inf = new InformeExaminar;
            hacer_examinar(ex, *inf);
            enviar(INFORME_EXAMINAR, inf);
            continue;
        }
        auto* inf = new InformePasada;
        hacer_pasada(archivo, *inf);
        {
            std::lock_guard<std::mutex> lk(h.m);
            h.terminadas = std::max(h.terminadas, numero);
        }
        h.cv_fin.notify_all();
        enviar(INFORME_PASADA, inf);
    }
}

}  // namespace

void iniciar_hilo(HWND ventana) {
    hilo().ventana = ventana;
    std::thread(bucle).detach();   // nunca join: esperaria a la red
}

unsigned long long pedir_pasada(const std::wstring& archivo) {
    Hilo& h = hilo();
    unsigned long long n;
    {
        std::lock_guard<std::mutex> lk(h.m);
        h.pasada = true;
        h.archivo = archivo;
        n = ++h.pedidas;
    }
    h.cv.notify_one();
    return n;
}

bool esperar_pasada(unsigned long long numero, DWORD ms) {
    Hilo& h = hilo();
    std::unique_lock<std::mutex> lk(h.m);
    return h.cv_fin.wait_for(lk, std::chrono::milliseconds(ms), [&] { return h.terminadas >= numero; });
}

void examinar(const std::wstring& carpeta) {
    Hilo& h = hilo();
    {
        std::lock_guard<std::mutex> lk(h.m);
        h.examinar.push_back(carpeta);
    }
    h.cv.notify_one();
}

}  // namespace datos
