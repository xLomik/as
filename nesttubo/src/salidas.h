// NestTubo - archivos de salida escritos a mano, sin librerias y sin Win32:
// informe PDF, libro Excel (.xlsx) y un DXF por distribucion de barra.
#pragma once

#include <string>
#include <vector>

#include "trabajo.h"

namespace nt {

// Informe PDF (A4): primera pagina con el resumen y los parametros, despues el
// plan de cada perfil con el dibujo de cada barra. Devuelve los bytes del archivo.
std::string informe_pdf(const std::vector<ResultadoPerfil>& rs, const std::string& trabajo, const std::string& fecha);

// Libro Excel: hojas Resumen, Plan (una fila por pieza) y Piezas (lo digitado).
std::string informe_xlsx(const std::vector<ResultadoPerfil>& rs, const std::string& trabajo, const std::string& fecha);

// Un DXF por distribucion distinta de barra (barras iguales comparten archivo,
// con "xN" en el nombre). Dibujo de la cara mas ancha, en mm, para AutoCAD (R12).
struct ArchivoDxf {
    std::string nombre;   // nombre de archivo, sin tildes ni caracteres prohibidos en Windows
    std::string texto;
};
std::vector<ArchivoDxf> dxf_perfil(const ResultadoPerfil& r);

// Cara que se dibuja cuando el perfil no la trae (mm).
constexpr int CARA_SIN_DATO = 50;

// Utilidades expuestas para las pruebas
std::string a_winansi(const std::string& utf8);   // bytes cp1252; lo que no existe ahi sale como '?'
std::string sin_tildes(const std::string& utf8);  // ASCII: a, n, x... para DXF y nombres de archivo
std::string zip_sin_comprimir(const std::vector<std::pair<std::string, std::string>>& archivos);

}  // namespace nt
