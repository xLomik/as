// NestTubo - carpeta de datos (ver datos.h).
#include "datos.h"

#include <shlobj.h>

#include <algorithm>
#include <chrono>
#include <condition_variable>
#include <cstdio>
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

// Junto a cada trabajo en espera va su base (ver Sello) en "<archivo>.ntbase".
std::wstring ruta_base(const std::wstring& en_espera) { return en_espera + L".ntbase"; }

std::string texto_base(const Sello& s) {
    char b[96];
    std::snprintf(b, sizeof b, "NESTTUBO-BASE 1\n%d %d %llu %llu\n", s.conocido ? 1 : 0, s.existe ? 1 : 0, s.hora, s.tamano);
    return b;
}

Sello leer_base(const std::wstring& en_espera) {
    std::string d;
    Sello s;
    int c = 0, e = 0;
    unsigned long long h = 0, t = 0;
    if (leer_archivo(ruta_base(en_espera), d) == Lectura::ok &&
        std::sscanf(d.c_str(), "NESTTUBO-BASE 1 %d %d %llu %llu", &c, &e, &h, &t) == 4) {
        s.conocido = c != 0;
        s.existe = e != 0;
        s.hora = h;
        s.tamano = t;
    }
    return s;
}

void borrar_carpetas_vacias(std::wstring c) {
    std::wstring tope = clave_ruta(carpeta_espera());
    while (clave_ruta(c).size() > tope.size() && clave_ruta(c).compare(0, tope.size(), tope) == 0) {
        if (!RemoveDirectoryW(c.c_str())) break;
        c = carpeta_de(c);
    }
}

// Borra un trabajo en espera y su base (con NestTubo-local tomado).
void borrar_en_espera(const std::wstring& f) {
    DeleteFileW(f.c_str());
    DeleteFileW(ruta_base(f).c_str());
    borrar_carpetas_vacias(carpeta_de(f));
}

// "<carpeta>\<nombre> (guardado sin conexión).ntb" que aun no exista ("" si no hay).
std::wstring nombre_sin_conexion(const std::wstring& destino) {
    std::wstring c = con_barra(carpeta_de(destino)), n = nombre_de(destino), ext;
    size_t k = n.find_last_of(L'.');
    if (k != std::wstring::npos && k > 0) {
        ext = n.substr(k);
        n = n.substr(0, k);
    }
    for (int i = 1; i < 100; i++) {
        std::wstring r = c + n + L" (guardado sin conexión" + (i > 1 ? L" " + std::to_wstring(i) : L"") + L")" + ext;
        if (!existe(r)) return r;
    }
    return L"";
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

// reemplazar = false: si ya hay un archivo con ese nombre no se toca y el codigo
// es ERROR_ALREADY_EXISTS (para no pisar uno que aparecio despues de mirar).
static bool escribir_(const std::wstring& ruta, const std::string& datos, DWORD* codigo, bool reemplazar) {
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
            if (ok && MoveFileExW(tmp.c_str(), ruta.c_str(), (reemplazar ? MOVEFILE_REPLACE_EXISTING : 0) | MOVEFILE_WRITE_THROUGH)) {
                if (codigo) *codigo = 0;
                return true;
            }
            if (ok) e = GetLastError();
            DeleteFileW(tmp.c_str());
        }
        if (e != ERROR_SHARING_VIOLATION && e != ERROR_ACCESS_DENIED && e != ERROR_LOCK_VIOLATION) break;
    }
    if (e == ERROR_FILE_EXISTS) e = ERROR_ALREADY_EXISTS;
    if (codigo) *codigo = e ? e : ERROR_WRITE_FAULT;
    return false;
}

bool escribir_archivo(const std::wstring& ruta, const std::string& datos, DWORD* codigo) { return escribir_(ruta, datos, codigo, true); }

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

bool dentro_de_espera(const std::wstring& ruta) {
    std::wstring r = clave_ruta(ruta), e = clave_ruta(carpeta_espera());
    return r == e || (r.size() > e.size() && r.compare(0, e.size(), e) == 0 && r[e.size()] == L'\\');
}

bool destino_valido(const std::wstring& destino) {
    std::wstring c = carpeta_de(destino);
    return !nombre_de(destino).empty() && !raiz(destino).empty() && !c.empty() && !raiz(c).empty() && !dentro_de_espera(destino);
}

Sello sello_de(const std::wstring& ruta) {
    Sello s;
    WIN32_FILE_ATTRIBUTE_DATA a;
    if (!GetFileAttributesExW(ruta.c_str(), GetFileExInfoStandard, &a)) {
        DWORD e = GetLastError();
        s.conocido = e == ERROR_FILE_NOT_FOUND || e == ERROR_PATH_NOT_FOUND;
        return s;
    }
    s.conocido = true;
    s.existe = true;
    if (a.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) return s;   // una carpeta con ese nombre: nunca es "el mismo"
    s.hora = ((unsigned long long)a.ftLastWriteTime.dwHighDateTime << 32) | a.ftLastWriteTime.dwLowDateTime;
    s.tamano = ((unsigned long long)a.nFileSizeHigh << 32) | a.nFileSizeLow;
    return s;
}

Sello sello_nuevo() {
    Sello s;
    s.conocido = true;
    return s;
}

bool mismo_archivo(const Sello& a, const Sello& b) {
    if (!a.conocido || !b.conocido || a.existe != b.existe) return false;
    return !a.existe || (a.hora == b.hora && a.tamano == b.tamano && a.hora != 0);
}

Sello base_en_espera(const std::wstring& destino) {
    std::wstring p = ruta_en_espera(destino);
    if (p.empty()) return Sello{};
    Bloqueo b;
    return leer_base(p);
}

namespace {

// conocidos.txt: una linea "hora tamano ruta" por trabajo de red, el mas reciente al final.
std::wstring ruta_conocidos() { return local_() + L"\\conocidos.txt"; }
const size_t MAX_CONOCIDOS = 300;

std::vector<std::pair<std::wstring, Sello>> leer_conocidos() {
    std::vector<std::pair<std::wstring, Sello>> v;
    std::string d;
    if (leer_archivo(ruta_conocidos(), d) != Lectura::ok) return v;
    size_t i = 0;
    while (i < d.size()) {
        size_t f = d.find('\n', i);
        if (f == std::string::npos) f = d.size();
        std::string linea = d.substr(i, f - i);
        i = f + 1;
        unsigned long long h = 0, t = 0;
        int n = 0;
        if (std::sscanf(linea.c_str(), "%llu %llu %n", &h, &t, &n) != 2 || n <= 0 || (size_t)n >= linea.size() || h == 0) continue;
        Sello s;
        s.conocido = s.existe = true;
        s.hora = h;
        s.tamano = t;
        v.push_back({W(linea.substr(n)), s});
    }
    return v;
}

// Con NestTubo-local tomado.
Sello conocido_(const std::wstring& destino) {
    std::wstring k = clave_ruta(destino);
    auto v = leer_conocidos();
    for (size_t i = v.size(); i-- > 0;)
        if (clave_ruta(v[i].first) == k) return v[i].second;
    return Sello{};
}

void recordar_(const std::wstring& destino, const Sello& s) {
    if (!s.conocido || !s.existe || s.hora == 0 || !es_remota(destino)) return;
    std::wstring k = clave_ruta(destino);
    auto v = leer_conocidos();
    v.erase(std::remove_if(v.begin(), v.end(), [&](const std::pair<std::wstring, Sello>& x) { return clave_ruta(x.first) == k; }), v.end());
    v.push_back({normal(destino), s});
    if (v.size() > MAX_CONOCIDOS) v.erase(v.begin(), v.begin() + (v.size() - MAX_CONOCIDOS));
    std::string texto = "NESTTUBO-CONOCIDOS 1\n";
    char b[64];
    for (const auto& x : v) {
        std::snprintf(b, sizeof b, "%llu %llu ", x.second.hora, x.second.tamano);
        texto += b + U(x.first) + "\n";
    }
    escribir_archivo(ruta_conocidos(), texto);
}

}  // namespace

Sello conocido(const std::wstring& destino) {
    Bloqueo b;
    return conocido_(destino);
}

void recordar(const std::wstring& destino, const Sello& s) {
    Bloqueo b;
    recordar_(destino, s);
}

bool guardar_trabajo(const std::wstring& destino, const std::string& texto, const Sello& base, bool es_el_abierto, bool& en_espera,
                     Sello& sello, std::wstring& error) {
    en_espera = false;
    sello = base;
    if (clave_ruta(nombre_de(destino)) == L"perfiles.ntb") {
        error = L"\"perfiles.ntb\" es el nombre del catálogo de perfiles. Guarda el trabajo con otro nombre.";
        return false;
    }
    if (dentro_de_espera(destino)) {
        error = L"Esa carpeta es donde NestTubo deja los trabajos que esperan la red:\n" + carpeta_espera() +
                L"\n\nGuarda el trabajo en otra carpeta.";
        return false;
    }
    if (!destino_valido(destino)) {
        error = L"No se puede guardar en\n" + destino + L"\n\nElige una carpeta dentro de una unidad o de una carpeta compartida.";
        return false;
    }
    std::wstring p = ruta_en_espera(destino);
    DWORD cod = 0;
    if (es_remota(destino)) {
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
        if (existe(p) && existe(ruta_base(p))) {
            sello = leer_base(p);   // la de la primera vez que se guardo sin copiar: lo que habia en la red antes
        } else {
            // la pasada pudo copiar el trabajo abierto despues de lo ultimo que supo la ventana
            Sello k = es_el_abierto ? conocido_(destino) : Sello{};
            if (k.conocido) sello = k;
            if (!escribir_archivo(ruta_base(p), texto_base(sello), &cod)) {
                error = L"No se pudo guardar en este equipo:\n" + ruta_base(p) + L"\n\n" + texto_error(cod);
                return false;
            }
        }
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
    sello = sello_de(destino);
    if (!p.empty()) {   // una version en espera (letra que volvio como disco local) ya es mas vieja que esta
        Bloqueo b;
        if (existe(p) || existe(ruta_base(p))) borrar_en_espera(p);
    }
    return true;
}

std::wstring version_a_abrir(const std::wstring& ruta, std::wstring& destino, bool& en_espera, Sello& base) {
    en_espera = false;
    destino = ruta;
    base = Sello{};
    std::wstring d;
    if (destino_en_espera(ruta, d)) {
        destino = d;
        en_espera = true;
        Bloqueo b;
        base = leer_base(ruta);
        return ruta;
    }
    // aunque hoy no sea de red: una letra pudo volver como disco local con una version en espera
    std::wstring p = ruta_en_espera(ruta);
    Bloqueo b;
    if (!p.empty() && existe(p)) {
        en_espera = true;
        base = leer_base(p);
        return p;
    }
    return ruta;
}

// ---------------------------------------------------------------------------
// Catalogo

namespace {

// Copia local con los pendientes aplicados, con NestTubo-local tomado. false si
// la copia existe y no se puede leer (nunca se trata como vacia).
bool leer_vigente(std::vector<PerfilTxt>& catalogo, Lectura& l, std::wstring& motivo) {
    catalogo.clear();
    if (!leer_catalogo(ruta_copia(), catalogo, l, motivo) && l == Lectura::error) return false;
    if (!leer_config(L"carpeta_datos").empty()) {
        std::string d, e;
        CambiosCatalogo p;
        if (leer_archivo(ruta_pendientes(), d) == Lectura::ok && de_texto_cambios(d, p, e)) catalogo = aplicar_cambios(catalogo, p);
    }
    return true;
}

}  // namespace

bool cargar_catalogo(std::vector<PerfilTxt>& catalogo, std::wstring& error) {
    Bloqueo b;
    std::wstring copia = ruta_copia();
    if (!existe(copia)) {   // migracion desde 0.2: el catalogo vivia en %APPDATA%
        std::wstring vieja = config_() + L"\\perfiles.ntb";
        if (clave_ruta(vieja) != clave_ruta(copia) && existe(vieja)) CopyFileW(vieja.c_str(), copia.c_str(), TRUE);
    }
    Lectura l;
    std::wstring motivo;
    if (leer_vigente(catalogo, l, motivo)) return true;
    error = L"No se pudo leer " + copia + L": " + motivo;
    return false;
}

std::vector<PerfilTxt> catalogo_vigente(const std::vector<PerfilTxt>& si_falla) {
    Bloqueo b;
    std::vector<PerfilTxt> c;
    Lectura l;
    std::wstring motivo;
    return leer_vigente(c, l, motivo) ? c : si_falla;
}

bool registrar(const Cambio& cambio, std::vector<PerfilTxt>& catalogo, std::wstring& error) {
    error.clear();
    Bloqueo b;
    // se parte de la copia en disco: la pasada pudo cambiarla
    std::vector<PerfilTxt> base;
    Lectura l;
    std::wstring motivo;
    if (!leer_catalogo(ruta_copia(), base, l, motivo) && l == Lectura::error) {
        error = L"No se pudo leer el catálogo de perfiles de este equipo:\n" + ruta_copia() + L"\n\n" + motivo;
        return false;
    }
    bool hay_carpeta = !leer_config(L"carpeta_datos").empty();
    std::string d, e;
    CambiosCatalogo pend;
    Lectura lp = hay_carpeta ? leer_archivo(ruta_pendientes(), d) : Lectura::no_existe;
    bool pend_ok = lp == Lectura::ok && de_texto_cambios(d, pend, e);
    if (pend_ok) base = aplicar_cambios(base, pend);   // por si se corto entre las dos escrituras
    catalogo = base;
    CambiosCatalogo c = cambio(base);
    if (c.vacio()) return true;
    std::vector<PerfilTxt> resultado = aplicar_cambios(base, c);
    DWORD cod = 0;
    if (hay_carpeta) {
        // primero los pendientes: si se corta antes de la copia local, al arrancar se aplican
        if (!pend_ok) {
            pend = CambiosCatalogo{};
            if (lp != Lectura::no_existe) pend.cambiados = base;   // ilegibles: se manda todo el catalogo
        }
        juntar_cambios(pend, c);
        if (!escribir_archivo(ruta_pendientes(), a_texto_cambios(pend), &cod)) {
            error = L"No se pudieron anotar los cambios de perfiles en\n" + ruta_pendientes() + L"\n\n" + texto_error(cod);
            return false;
        }
    }
    if (!escribir_archivo(ruta_copia(), texto_catalogo(resultado), &cod)) {
        error = L"No se pudo guardar el catálogo de perfiles en\n" + ruta_copia() + L"\n\n" + texto_error(cod);
        if (!hay_carpeta) return false;
        // los pendientes lo tienen: el catalogo vigente ya lo incluye y la pasada reescribe la copia
        error += L"\n\nEl cambio quedó anotado para la carpeta de datos.";
        catalogo = resultado;
        return true;
    }
    catalogo = resultado;
    return true;
}

bool registrar(const CambiosCatalogo& c, std::vector<PerfilTxt>& catalogo, std::wstring& error) {
    if (c.vacio()) return true;
    return registrar([&](const std::vector<PerfilTxt>&) { return c; }, catalogo, error);
}

bool cambiar_carpeta(const std::wstring& carpeta, Lectura lectura, const std::vector<PerfilTxt>& de_la_carpeta, bool gana_carpeta,
                     std::vector<PerfilTxt>& resultado, std::wstring& error) {
    Bloqueo b;
    std::wstring copia = ruta_copia();
    std::vector<PerfilTxt> local;   // con los pendientes: un cambio que solo esta ahi tambien cuenta
    Lectura l;
    std::wstring motivo;
    if (!leer_vigente(local, l, motivo)) {
        error = L"No se pudo leer el catálogo de perfiles de este equipo:\n" + copia + L"\n\n" + motivo;
        return false;
    }
    bool habia = lectura == Lectura::ok;
    resultado = !habia ? local : gana_carpeta ? unir_catalogos(de_la_carpeta, local) : unir_catalogos(local, de_la_carpeta);
    DWORD cod = 0;
    if (habia && !local.empty()) {
        std::wstring resp = local_() + L"\\perfiles antes de cambiar de carpeta.ntb";
        if (!escribir_archivo(resp, texto_catalogo(local), &cod)) {
            error = L"No se pudo respaldar el catálogo de este equipo en " + resp + L": " + texto_error(cod);
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

// Solo para pruebas: NESTTUBO_INTERVALO_S cambia los 30 s entre pasadas solas.
unsigned intervalo_s() {
    static const unsigned d = [] {
        wchar_t v[32] = L"";
        DWORD n = GetEnvironmentVariableW(L"NESTTUBO_INTERVALO_S", v, 32);
        unsigned x = n && n < 32 ? (unsigned)wcstoul(v, nullptr, 10) : 0u;
        return x ? x : 30u;
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
        else {
            std::wstring k = clave_ruta(n);
            bool termina = false;
            for (const wchar_t* x : {L".tmp", L".ntbase"}) {
                size_t m = wcslen(x);
                termina = termina || (k.size() > m && k.compare(k.size() - m, m, x) == 0);
            }
            if (!termina) archivos.push_back(r);
        }
    } while (FindNextFileW(h, &f));
    FindClose(h);
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
    bool subidos = lp == Lectura::ok && l2 == Lectura::ok && ahora == pend_bytes;   // nadie anoto nada mientras tanto
    if (!subidos && l2 == Lectura::ok && !de_texto_cambios(ahora, quedan, e)) quedan = CambiosCatalogo{};
    std::vector<PerfilTxt> nuevo = aplicar_cambios(p.resultado, quedan);
    std::string texto = texto_catalogo(nuevo), actual;
    DWORD cod = 0;
    if ((leer_archivo(ruta_copia(), actual) != Lectura::ok || actual != texto) && !escribir_archivo(ruta_copia(), texto, &cod)) {
        // los pendientes se quedan: sin la copia al dia son lo unico que tiene los cambios en este equipo
        inf.error_catalogo = L"No se pudo actualizar el catálogo de este equipo (" + ruta_copia() + L"): " + texto_error(cod);
        inf.catalogo_en_espera = true;
        return;
    }
    if (subidos) DeleteFileW(ruta_pendientes().c_str());
    inf.hay_catalogo = true;
    inf.catalogo_en_espera = existe(ruta_pendientes());
}

void hacer_pasada(const std::wstring& archivo, InformePasada& inf) {
    std::vector<std::wstring> espera;
    listar(carpeta_espera(), espera);
    std::wstring carpeta = leer_config(L"carpeta_datos");
    inf.carpeta_datos = carpeta;
    std::vector<std::wstring> rutas = {carpeta, leer_config(L"carpeta"), leer_config(L"carpeta_exportar"), archivo};
    std::vector<std::wstring> destinos(espera.size());
    for (size_t i = 0; i < espera.size(); i++) {
        // un archivo de la espera sin destino valido no es un trabajo que esperar
        if (!destino_en_espera(espera[i], destinos[i]) || !destino_valido(destinos[i]) ||
            clave_ruta(nombre_de(destinos[i])) == L"perfiles.ntb") {
            destinos[i].clear();
        } else {
            rutas.push_back(destinos[i]);
        }
    }
    HANDLE sinc = CreateMutexW(nullptr, FALSE, L"NestTubo-sincronizar");
    if (sinc) {
        DWORD r = WaitForSingleObject(sinc, 0);
        if (r != WAIT_OBJECT_0 && r != WAIT_ABANDONED) {
            CloseHandle(sinc);
            inf.saltada = true;
            return;
        }
    }
    // 1. que raices responden
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
    std::map<std::wstring, int> sin_red;
    for (size_t i = 0; i < espera.size(); i++) {
        const std::wstring& f = espera[i];
        const std::wstring& destino = destinos[i];
        if (destino.empty()) continue;
        std::wstring k = clave_ruta(raiz(destino));
        if (!en_linea[k]) {
            inf.trabajos_en_espera++;
            sin_red[k]++;
            continue;
        }
        std::string bytes;
        Sello base;
        Lectura lf;
        {
            Bloqueo b;
            lf = leer_archivo(f, bytes);
            base = leer_base(f);
        }
        if (lf != Lectura::ok) {
            if (lf == Lectura::error) inf.trabajos_en_espera++;
            continue;
        }
        esperar_red();
        Sello actual = sello_de(destino);
        if (!actual.conocido) {
            DWORD cod = GetLastError();
            if (!responde(raiz(destino))) {
                en_linea[k] = false;
                sin_red[k]++;
            } else {
                inf.fallas.push_back(Falla{f, destino, cod});
            }
            inf.trabajos_en_espera++;
            continue;
        }
        // en la red hay otro archivo (otro trabajo con ese nombre, o cambio mientras tanto): no se pisa
        std::wstring escrito = destino;
        if (actual.existe && !mismo_archivo(base, actual)) escrito = nombre_sin_conexion(destino);
        if (escrito.empty()) {
            inf.fallas.push_back(Falla{f, destino, ERROR_FILE_EXISTS});
            inf.trabajos_en_espera++;
            continue;
        }
        crear_carpetas(carpeta_de(escrito));
        DWORD cod = 0;
        // donde no habia nada (o al lado) se escribe sin reemplazar: si aparecio otro archivo, tampoco se pisa
        bool ok = escribir_(escrito, bytes, &cod, escrito == destino && actual.existe);
        if (!ok && cod == ERROR_ALREADY_EXISTS) {
            escrito = nombre_sin_conexion(destino);
            ok = !escrito.empty() && escribir_(escrito, bytes, &cod, false);
        }
        if (ok) {
            Copiado c{destino, escrito, sello_de(escrito), false};
            Bloqueo b;
            recordar_(escrito, c.sello);   // la base del proximo Guardar de este trabajo (ver guardar_trabajo)
            std::string ahora;
            if (leer_archivo(f, ahora) == Lectura::ok && ahora != bytes) {   // se guardo otra vez: va en la siguiente
                c.sigue_en_espera = true;
                inf.trabajos_en_espera++;
                if (escrito == destino) escribir_archivo(ruta_base(f), texto_base(c.sello));   // parte de lo que se acaba de copiar
            } else {
                borrar_en_espera(f);
            }
            inf.copiados.push_back(c);
        } else {
            if (!responde(raiz(destino))) {
                en_linea[k] = false;
                sin_red[k]++;
            } else {
                inf.fallas.push_back(Falla{f, destino, cod});
            }
            inf.trabajos_en_espera++;
        }
    }
    for (const auto& x : sin_red) inf.esperando_red.push_back({nombre[x.first], x.second});
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
            if (!h.pasada && h.examinar.empty()) h.cv.wait_for(lk, std::chrono::seconds(intervalo_s()));
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
