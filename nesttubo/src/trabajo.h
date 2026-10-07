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

// Columnas del resumen: perfil, barra, barras a enviar, minimo calculado,
// margen, piezas, aprovechamiento, minimo demostrado.
std::vector<std::string> columnas_resumen();
std::vector<std::string> fila_resumen(const ResultadoPerfil& r);

// Plan por barra: una fila por pieza, barras iguales agrupadas, inicio y fin
// de cada pieza y sobrante de la barra. Al final, las piezas que no caben.
std::vector<std::string> columnas_plan();
std::vector<std::vector<std::string>> filas_plan(const ResultadoPerfil& r);

// Archivo de trabajo: texto UTF-8 separado por tabuladores.
std::string a_texto(const Trabajo& t);
bool de_texto(const std::string& texto, Trabajo& t, std::string& error);

}  // namespace nt
