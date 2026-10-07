// NestTubo - el trabajo tal como lo digita Camilo (texto), su lectura a numeros
// y los textos del resultado. Sin Win32: todo en UTF-8 y probado nativo.
#pragma once

#include <string>
#include <vector>

#include "nucleo.h"

namespace nt {

// Valores iniciales de un perfil nuevo (CLAUDE.md: editables por perfil).
constexpr const char* BARRA_INICIAL = "6000";
constexpr const char* DESPUNTE_INICIAL = "10";
constexpr const char* ZONA_MUERTA_INICIAL = "230";
constexpr const char* SEPARACION_INICIAL = "3";
constexpr const char* MARGEN_INICIAL = "0";

// Una fila de cada tabla, tal como se digito.
struct PerfilTxt {
    std::string nombre, cara, barra, despunte, zona_muerta, separacion, margen;
};
struct PiezaTxt {
    std::string perfil, nombre, largo, angulo1, angulo2, cantidad;
};
struct Trabajo {
    std::vector<PerfilTxt> perfiles;
    std::vector<PiezaTxt> piezas;
};

PerfilTxt perfil_nuevo(const std::string& nombre);
bool fila_vacia(const PiezaTxt& p);
bool fila_vacia(const PerfilTxt& p);

// Un problema por perfil, ya en enteros. escala = 1 (mm) o 10 (decimas de mm).
struct ProblemaPerfil {
    std::string nombre;
    int escala = 1;
    Parametros param;
    i64 cara = 0;                       // medida de la cara mas ancha (0 si no se dio)
    int margen = 0;                     // barras extra a enviar
    std::vector<Pieza> piezas;          // id = indice de la fila en Trabajo::piezas
    std::vector<std::string> nombres;   // nombre de cada fila de piezas (por id)
    std::vector<i64> angulo1, angulo2;  // angulo de cada extremo por id, en decimas de grado (0 = recto)
};

// Lee las dos tablas. Devuelve false y llena `errores` (uno por linea, con la
// fila) si algo no se puede leer; en ese caso no se calcula nada.
bool preparar(const Trabajo& t, std::vector<ProblemaPerfil>& problemas, std::vector<std::string>& errores);

// Medida en la unidad interna -> texto en mm con coma decimal ("1234,5", "2000").
std::string medida(i64 v, int escala);

// Lo que se muestra del resultado de un perfil.
struct ResultadoPerfil {
    ProblemaPerfil prob;
    Plan plan;
    bool valido = false;
    std::string motivo;   // si no es valido, por que
    i64 barras_enviar() const { return (i64)plan.barras.size() + prob.margen; }
};

// La pieza de un id (nullptr si no esta).
const Pieza* pieza_de(const ProblemaPerfil& p, int id);

// Nombre de una pieza que no cabe, con su cantidad si es mas de una: "Larguero (× 8)".
std::string nombre_no_cabe(const ProblemaPerfil& p, int id);

// Calcula un perfil y pasa el plan por el validador. Nunca lanza: un error
// queda en `motivo` con valido = false.
ResultadoPerfil resolver(const ProblemaPerfil& prob, const Control* control = nullptr);

// Columnas del resumen: perfil, barra, barras a enviar, minimo calculado,
// margen, piezas, aprovechamiento, minimo demostrado.
std::vector<std::string> columnas_resumen();
std::vector<std::string> fila_resumen(const ResultadoPerfil& r);

// Plan por barra: una fila por pieza, barras iguales agrupadas, inicio y fin
// de cada pieza y sobrante de la barra. Al final, las piezas que no caben.
std::vector<std::string> columnas_plan();
std::vector<std::vector<std::string>> filas_plan(const ResultadoPerfil& r);

// ---------------------------------------------------------------------------
// Catalogo de perfiles compartido entre equipos (carpeta de datos).
// Cada equipo manda solo lo que cambio a mano: perfiles editados o agregados y
// perfiles quitados. Asi dos equipos no se pisan y abrir un trabajo viejo no
// cambia el catalogo.

// Nombre de perfil para comparar: sin espacios en los extremos y sin distinguir
// mayusculas (como al leer las piezas).
std::string clave_perfil(const std::string& nombre);

struct CambiosCatalogo {
    std::vector<PerfilTxt> cambiados;   // valores nuevos; reemplazan por nombre o se agregan al final
    std::vector<std::string> quitados;  // nombres que se borran
    bool vacio() const { return cambiados.empty() && quitados.empty(); }
};

// Junta `nuevos` sobre `viejos` (lo de `nuevos` gana para el mismo nombre).
void juntar_cambios(CambiosCatalogo& viejos, const CambiosCatalogo& nuevos);

// El catalogo con los cambios aplicados. Las filas vacias y los nombres repetidos
// del catalogo se limpian (queda la primera).
std::vector<PerfilTxt> aplicar_cambios(const std::vector<PerfilTxt>& catalogo, const CambiosCatalogo& c);

// Posicion del perfil con ese nombre (clave_perfil) o -1.
int buscar_perfil(const std::vector<PerfilTxt>& v, const std::string& nombre);

// Campo `col` (0 nombre, 1 cara, 2 barra, 3 despunte, 4 zona muerta, 5 separacion, 6 margen).
std::string* campo_perfil(PerfilTxt& p, int col);

// Lo que cambia en el catalogo al editar a mano la columna `col` (1 a 6) de una
// fila de perfiles que ya tiene nombre. `fila` ya trae el valor nuevo. Si el
// perfil esta en el catalogo se manda su fila del catalogo con solo ese campo
// cambiado: asi corregir un campo en un trabajo viejo no sube sus otros valores
// viejos. Si no esta, se manda la fila de la tabla.
CambiosCatalogo cambio_por_campo(const std::vector<PerfilTxt>& catalogo, const PerfilTxt& fila, int col);

// Lo que cambia en el catalogo al cambiar a mano el nombre de una fila de
// perfiles de `viejo` a `fila.nombre`. `viejo_sigue_en_tabla`: otra fila aun
// tiene el nombre viejo (entonces no se quita del catalogo).
CambiosCatalogo cambio_por_nombre(const std::vector<PerfilTxt>& catalogo, const PerfilTxt& fila, const std::string& viejo,
                                  bool viejo_sigue_en_tabla);

// Nombres que estan en las dos listas con algun campo distinto (sin contar
// espacios en los extremos).
std::vector<std::string> perfiles_distintos(const std::vector<PerfilTxt>& a, const std::vector<PerfilTxt>& b);

// Una pasada de sincronizacion del catalogo con la carpeta de datos.
enum class Lectura { ok, no_existe, error };
struct PasadaCatalogo {
    bool escribir = false;            // escribir `resultado` en la carpeta
    std::vector<PerfilTxt> resultado; // catalogo despues de la pasada (vacio si hubo error)
};
// error: no se escribe nada (un archivo que no se pudo leer nunca cuenta como vacio).
// no_existe: se parte de la copia local. ok: se parte del de la carpeta.
PasadaCatalogo pasada_catalogo(Lectura lectura, const std::vector<PerfilTxt>& carpeta, const std::vector<PerfilTxt>& local,
                               const CambiosCatalogo& pendientes);

// Trabajos guardados sin conexion: copia local que repite la ruta del destino.
//   \\srv\rec\a\b.ntb -> <raiz>\red\srv\rec\a\b.ntb      Z:\a\b.ntb -> <raiz>\Z\a\b.ntb
// Otra forma de ruta (\\?\, relativa) -> "". Todo en UTF-8 con '\\'.
std::string ruta_pendiente(const std::string& raiz, const std::string& destino);
bool destino_de_pendiente(const std::string& raiz, const std::string& ruta, std::string& destino);

// Primera vez con una carpeta que ya tiene catalogo: gana la carpeta para el
// mismo nombre y se agregan los perfiles que solo tiene este equipo.
std::vector<PerfilTxt> unir_catalogos(const std::vector<PerfilTxt>& carpeta, const std::vector<PerfilTxt>& local);

// Archivo de cambios pendientes (cuando la carpeta no responde).
std::string a_texto_cambios(const CambiosCatalogo& c);
bool de_texto_cambios(const std::string& texto, CambiosCatalogo& c, std::string& error);

// Archivo de trabajo: texto UTF-8 separado por tabuladores.
std::string a_texto(const Trabajo& t);
bool de_texto(const std::string& texto, Trabajo& t, std::string& error);

}  // namespace nt
