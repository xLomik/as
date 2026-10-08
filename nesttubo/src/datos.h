// NestTubo - carpeta de datos: configuracion, archivos, catalogo de perfiles y
// el hilo que copia a la red. Win32 pero sin ventanas: la ventana solo llama
// estas funciones y recibe los informes del hilo con WM_APP_RED.
//
// Donde va cada cosa:
//   %APPDATA%\NestTubo\config.ini           carpeta, carpeta_exportar, carpeta_datos
//   %LOCALAPPDATA%\NestTubo\perfiles.ntb    copia local del catalogo
//   %LOCALAPPDATA%\NestTubo\perfiles_pendientes.ntb   cambios que aun no llegan a la carpeta
//   %LOCALAPPDATA%\NestTubo\Sin conexión\   trabajos de red guardados primero en este equipo
//   %LOCALAPPDATA%\NestTubo\conocidos.txt   como dejo este equipo cada trabajo de red (ver Sello)
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
// Un trabajo de red tiene que estar dentro de una raiz: \\srv\rec\x.ntb o N:\x.ntb, no \\srv\x.ntb.
bool destino_valido(const std::wstring& destino);

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

// Como dejo este equipo un trabajo de red la ultima vez que lo leyo o lo escribio
// (al abrirlo, o la pasada al copiarlo). No depende de que el informe de la pasada
// llegue a tiempo a la ventana. conocido = false si no hay registro.
Sello conocido(const std::wstring& destino);
void recordar(const std::wstring& destino, const Sello& s);

// Escribe un trabajo. Si el destino es de red, lo deja en la carpeta de espera
// (en_espera = true) junto con su base y el hilo lo copia. Si no es de red se
// escribe directo y se borra la version en espera que hubiera (esta es mas nueva).
// La base de una version en espera nueva es `base`, salvo que `es_el_abierto` (se
// guarda el mismo archivo que la ventana tiene abierto) y este equipo lo haya
// copiado o leido despues: entonces es lo que dice conocido(). Si ya habia una
// version en espera, se conserva su base (lo que habia en la red antes).
// `sello`: la base que la ventana debe recordar para ese archivo desde ahora.
// false: no se pudo, con el motivo. "perfiles.ntb", la carpeta de espera y los
// destinos que no son validos no se aceptan.
bool guardar_trabajo(const std::wstring& destino, const std::string& texto, const Sello& base, bool es_el_abierto, bool& en_espera,
                     Sello& sello, std::wstring& error);

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

// Unico punto que cambia el catalogo. Bajo NestTubo-local lee el catalogo vigente,
// calcula el cambio con `cambio` sobre el (la pasada pudo cambiarlo: nunca se
// parte de lo que la ventana tiene en memoria), lo anota en los pendientes (si hay
// carpeta de datos) y despues en la copia local, y deja el resultado en `catalogo`.
// true: el cambio quedo (`error` puede traer un aviso: no se pudo escribir la copia
// local, pero los pendientes lo tienen). false: no quedo y no se escribio nada
// (copia local ilegible, o no se pudieron anotar los pendientes).
using Cambio = std::function<nt::CambiosCatalogo(const std::vector<nt::PerfilTxt>& actual)>;
bool registrar(const Cambio& cambio, std::vector<nt::PerfilTxt>& catalogo, std::wstring& error);
bool registrar(const nt::CambiosCatalogo& c, std::vector<nt::PerfilTxt>& catalogo, std::wstring& error);

// Cambio de carpeta de datos ya decidido. Bajo NestTubo-local vuelve a leer el
// catalogo vigente y lo junta con el de la carpeta (`lectura`, `de_la_carpeta`,
// gana la carpeta para el mismo nombre si `gana_carpeta`). El resultado pasa a ser
// la copia local y queda todo pendiente de subir. Si la carpeta tenia catalogo se
// guarda antes un respaldo del catalogo vigente.
bool cambiar_carpeta(const std::wstring& carpeta, nt::Lectura lectura, const std::vector<nt::PerfilTxt>& de_la_carpeta,
                     bool gana_carpeta, std::vector<nt::PerfilTxt>& resultado, std::wstring& error);

// Catalogo vigente: la copia local con los pendientes aplicados (un corte entre
// las dos escrituras, o una copia que no se pudo reemplazar, deja el cambio solo
// en los pendientes). `si_falla` si la copia no se puede leer.
std::vector<nt::PerfilTxt> catalogo_vigente(const std::vector<nt::PerfilTxt>& si_falla);

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
    bool saltada = false;            // otro proceso de NestTubo estaba en una pasada: nada de lo demas vale
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
