// NestTubo - carpeta de datos: configuracion, archivos, catalogo de perfiles y
// el hilo que copia a la red. Win32 pero sin ventanas: la ventana solo llama
// estas funciones y recibe los informes del hilo con WM_APP_RED.
//
// Donde va cada cosa:
//   %APPDATA%\NestTubo\config.ini           carpeta, carpeta_exportar, carpeta_datos
//   %LOCALAPPDATA%\NestTubo\perfiles.ntb    copia local del catalogo
//   %LOCALAPPDATA%\NestTubo\perfiles_pendientes.ntb   cambios que aun no llegan a la carpeta
//   %LOCALAPPDATA%\NestTubo\Sin conexión\   trabajos de red guardados primero en este equipo
//   <carpeta de datos>\perfiles.ntb         catalogo compartido
#pragma once

#include <windows.h>

#include <functional>
#include <string>
#include <vector>

#include "trabajo.h"

namespace datos {

std::wstring W(const std::string& s);   // UTF-8 -> UTF-16
std::string U(const std::wstring& s);   // UTF-16 -> UTF-8

// ---------------------------------------------------------------------------
// Configuracion y carpetas propias

std::wstring carpeta_config();   // %APPDATA%\NestTubo
std::wstring carpeta_local();    // %LOCALAPPDATA%\NestTubo
std::wstring carpeta_espera();   // %LOCALAPPDATA%\NestTubo\Sin conexión
std::wstring leer_config(const wchar_t* clave);
void escribir_config(const wchar_t* clave, const std::wstring& valor);
std::wstring carpeta_documentos();

// ---------------------------------------------------------------------------
// Archivos

// Un archivo que no existe es no_existe; uno que existe y no se puede leer
// entero es error (y nunca se trata como vacio). `codigo` = GetLastError.
nt::Lectura leer_archivo(const std::wstring& ruta, std::string& datos, DWORD* codigo = nullptr);
// Temporal en la misma carpeta y MoveFileExW: un corte no deja el archivo a
// medias. Reintenta 3 veces si el archivo esta en uso.
bool escribir_archivo(const std::wstring& ruta, const std::string& datos, DWORD* codigo = nullptr);
bool crear_carpetas(const std::wstring& carpeta);
bool existe(const std::wstring& ruta);
std::wstring carpeta_de(const std::wstring& ruta);   // sin la barra final
std::wstring nombre_de(const std::wstring& ruta);
std::wstring texto_error(DWORD codigo);

// \\srv\rec\..., una letra de red o una letra que hoy no existe. No cuentan \\?\ ni \\.\.
bool es_remota(const std::wstring& ruta);
// \\srv\rec o X:\ ("" si la ruta no tiene ninguna de las dos formas).
std::wstring raiz(const std::wstring& ruta);
// Para comparar raices y rutas sin distinguir mayusculas.
std::wstring clave_ruta(const std::wstring& ruta);

// ---------------------------------------------------------------------------
// Trabajos guardados sin conexion (en espera de copiarse a la red)

std::wstring ruta_en_espera(const std::wstring& destino);   // "" si no aplica
bool destino_en_espera(const std::wstring& ruta, std::wstring& destino);   // ruta dentro de la carpeta de espera
std::wstring carpeta_en_espera(const std::wstring& carpeta);   // la misma carpeta dentro de la de espera
bool dentro_de_espera(const std::wstring& ruta);   // la ruta esta en la carpeta de espera (o es ella)

// Como estaba el archivo de un trabajo la ultima vez que la ventana lo leyo o lo
// escribio. Al copiar lo guardado sin conexion, si el archivo de la red ya no esta
// asi (otro lo cambio, o es otro trabajo con el mismo nombre) no se reemplaza: lo
// guardado queda al lado como "<nombre> (guardado sin conexión).ntb".
struct Sello {
    bool conocido = false;   // false: no se sabe como estaba (se trata como cambiado)
    bool existe = false;
    unsigned long long hora = 0, tamano = 0;
};
Sello sello_de(const std::wstring& ruta);   // como esta ahora (toca la red si la ruta es de red)
Sello sello_nuevo();                         // un trabajo que no estaba en esa carpeta
bool mismo_archivo(const Sello& a, const Sello& b);
Sello base_en_espera(const std::wstring& destino);   // la base guardada con su version en espera

// Escribe un trabajo. Si el destino es de red, lo deja en la carpeta de espera
// (en_espera = true) junto con `base` y el hilo lo copia. Si no es de red se
// escribe directo y se borra la version en espera que hubiera (esta es mas nueva).
// `sello`: la base que la ventana debe recordar para ese archivo desde ahora.
// false: no se pudo, con el motivo. "perfiles.ntb" y la carpeta de espera no se aceptan.
bool guardar_trabajo(const std::wstring& destino, const std::string& texto, const Sello& base, bool& en_espera, Sello& sello,
                     std::wstring& error);

// Lo que hay que leer para abrir `ruta`: si es un archivo de la carpeta de espera,
// su destino pasa a ser `destino`; si tiene version en espera, se lee esa (y
// `base` es la que se guardo con ella).
std::wstring version_a_abrir(const std::wstring& ruta, std::wstring& destino, bool& en_espera, Sello& base);

// ---------------------------------------------------------------------------
// Catalogo de perfiles

// Al arrancar: migra la copia de 0.2 (%APPDATA%) si hace falta y devuelve la
// copia local con los cambios pendientes aplicados (por si se corto entre las
// dos escrituras). false si la copia local existe pero no se pudo leer.
bool cargar_catalogo(std::vector<nt::PerfilTxt>& catalogo, std::wstring& error);

// Unico punto que cambia el catalogo. Bajo NestTubo-local lee la copia local,
// calcula el cambio con `cambio` sobre esa copia (otra ventana o la pasada pudieron
// cambiarla: nunca se parte de lo que la ventana tiene en memoria), lo anota en
// los pendientes (si hay carpeta de datos) y despues en la copia local, y deja
// el resultado en `catalogo`. Si la copia local no se puede leer, o no se pueden
// anotar los pendientes, no se escribe nada.
using Cambio = std::function<nt::CambiosCatalogo(const std::vector<nt::PerfilTxt>& actual)>;
bool registrar(const Cambio& cambio, std::vector<nt::PerfilTxt>& catalogo, std::wstring& error);
bool registrar(const nt::CambiosCatalogo& c, std::vector<nt::PerfilTxt>& catalogo, std::wstring& error);

// Cambio de carpeta de datos ya decidido. Bajo NestTubo-local vuelve a leer la
// copia local y la junta con el catalogo de la carpeta (`lectura`, `de_la_carpeta`,
// gana la carpeta para el mismo nombre si `gana_carpeta`). El resultado pasa a ser
// la copia local y queda todo pendiente de subir. Si la carpeta tenia catalogo se
// guarda antes un respaldo de la copia local.
bool cambiar_carpeta(const std::wstring& carpeta, nt::Lectura lectura, const std::vector<nt::PerfilTxt>& de_la_carpeta,
                     bool gana_carpeta, std::vector<nt::PerfilTxt>& resultado, std::wstring& error);

// Copia local tal como esta en disco (para juntarla con la de otra carpeta).
std::vector<nt::PerfilTxt> copia_local(const std::vector<nt::PerfilTxt>& si_falla);

// ---------------------------------------------------------------------------
// Hilo de red: nunca lo espera la ventana (salvo hasta 3 s al cerrar).

enum : UINT { WM_APP_RED = WM_APP + 20 };   // wParam: INFORME_*, lParam: el informe (lo libera la ventana)
enum : WPARAM { INFORME_PASADA = 1, INFORME_EXAMINAR = 2 };

enum class Conexion { desconocida, en_linea, sin_conexion };

struct Falla {
    std::wstring en_espera, destino;
    DWORD codigo = 0;
};

struct Copiado {
    std::wstring destino;
    std::wstring escrito;        // != destino: en la red habia otro archivo y no se reemplazo
    Sello sello;                 // como quedo `escrito`
    bool sigue_en_espera = false;   // se guardo otra vez mientras se copiaba
};

struct InformePasada {
    std::vector<std::pair<std::wstring, Conexion>> raices;   // raiz -> estado
    std::wstring carpeta_datos;      // con la que se hizo la pasada
    bool hay_catalogo = false;       // la copia local se puso al dia con la carpeta (leerla con copia_local)
    std::wstring error_catalogo;     // no se pudo leer o escribir el catalogo de la carpeta
    bool catalogo_en_espera = false; // quedan cambios de perfiles sin subir
    std::vector<Copiado> copiados;   // trabajos copiados en esta pasada
    std::vector<Falla> fallas;            // errores reales (la raiz respondia)
    int trabajos_en_espera = 0;
    std::vector<std::pair<std::wstring, int>> esperando_red;   // raiz sin conexion -> trabajos que la esperan
    bool saltada = false;            // otra ventana de NestTubo estaba en una pasada
};

struct InformeExaminar {
    std::wstring carpeta;
    nt::Lectura lectura = nt::Lectura::error;
    std::vector<nt::PerfilTxt> catalogo;   // si lectura == ok
    std::wstring error;
};

void iniciar_hilo(HWND ventana);
// Pide una pasada (varias seguidas se juntan). `archivo`: el trabajo abierto, para
// saber si su raiz responde. Devuelve un numero para esperar_pasada.
unsigned long long pedir_pasada(const std::wstring& archivo);
bool esperar_pasada(unsigned long long numero, DWORD ms);
void examinar(const std::wstring& carpeta);

}  // namespace datos
