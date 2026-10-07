// NestTubo - nucleo de calculo (sin Win32).
//
// Cuantas barras de cada perfil hay que enviar al proveedor de corte laser.
// Un problema independiente por perfil:
//   util = largo_barra - despunte - zona_muerta
//   n piezas caben en una barra si  suma(largos) + (n-1)*separacion <= util
// Sumando la separacion a ambos lados queda un bin packing clasico:
//   pesos w = largo + separacion, capacidad C = util + separacion.
//
// Todo en enteros, en la "unidad interna" que elige quien llama (mm, o decimas
// de mm si algun dato trae decimal). La referencia de este port es
// laboratorio/simplex_propio.py.
#pragma once

#include <atomic>
#include <chrono>
#include <cstdint>
#include <map>
#include <string>
#include <vector>

namespace nt {

using i64 = long long;

// Cancelacion y tope de tiempo. Al vencer, el calculo entrega lo mejor que tenga.
struct Control {
    const std::atomic<bool>* cancelar = nullptr;
    std::chrono::steady_clock::time_point limite = std::chrono::steady_clock::time_point::max();
    bool cancelado() const { return cancelar && cancelar->load(std::memory_order_relaxed); }
    bool vencido() const { return cancelado() || std::chrono::steady_clock::now() >= limite; }
};

struct Info {
    int arranques = 0;
    int n_lp = 0;
    long long vueltas = 0;
    long long mochilas = 0;
    bool convergio = true;      // todos los LP que corrieron hasta el final convergieron
    bool interrumpido = false;  // se corto por tiempo o cancelacion
};

// ---------------------------------------------------------------------------
// Nivel 1: bin packing con pesos enteros.

using Barras = std::vector<std::vector<i64>>;   // pesos de cada barra

struct ResultadoPesos {
    Barras barras;
    i64 cota = 0;      // cota inferior valida: max(cota_lp, cota_l2)
    i64 cota_lp = 0;   // ceil(cota de Farley) del primer LP: la que trae el oraculo
    Info info;
};

// Metodo completo: heuristicas + LP por generacion de columnas + residual,
// clavado y reintentos. Lanza std::invalid_argument si algun peso es <= 0 o
// mayor que C (quien llama debe apartar antes las piezas que no caben).
ResultadoPesos resolver_pesos(const std::vector<i64>& w, i64 C, int arranques = 5,
                              const Control* control = nullptr);

// Piezas sueltas, expuestas para las pruebas. Las heuristicas devuelven
// barras como listas de indices de w.
i64 cota_l1(const std::vector<i64>& w, i64 C);
i64 cota_l2(const std::vector<i64>& w, i64 C);
std::vector<std::vector<int>> ffd(const std::vector<i64>& w, i64 C);
std::vector<std::vector<int>> bfd(const std::vector<i64>& w, i64 C);
std::vector<std::vector<int>> llenado(const std::vector<i64>& w, i64 C);

// Mochila acotada entera: max sum(v_j a_j), sum(w_j a_j) <= cap, 0 <= a_j <= tope_j.
double mochila(const std::vector<double>& valores, const std::vector<i64>& pesos,
               const std::vector<int>& tope, i64 cap, std::vector<int>& patron);

using Cantera = std::vector<std::map<i64, int>>;   // patrones como peso -> cantidad

struct ResultadoLP {
    i64 cota = 0;
    std::vector<std::vector<int>> patrones;   // solo los patrones de la base (sin holguras)
    std::vector<double> veces;
    bool convergio = false;
    bool interrumpido = false;
    long long vueltas = 0, mochilas = 0;
    double error_inversa = 0;                 // solo si se pide el diagnostico
};

// Relajacion lineal del modelo de patrones (Gilmore y Gomory), simplex
// revisado propio. `cantera` entra con patrones heredados y sale ampliada.
ResultadoLP lp_patrones(const std::vector<i64>& pesos, const std::vector<int>& dem, i64 cap,
                        Cantera* cantera, const Control* control = nullptr,
                        long long max_iter = 200000, bool diagnostico = false);

// Comprobacion de nivel 1: mismas piezas que las pedidas y ninguna barra pasada.
bool validar_pesos(const std::vector<i64>& w, i64 C, const Barras& barras, std::string* motivo = nullptr);

// ---------------------------------------------------------------------------
// Nivel 2: un perfil, en unidades reales (internas).

struct Parametros {
    i64 largo_barra = 0;
    i64 despunte = 0;
    i64 zona_muerta = 0;
    i64 separacion = 0;
    i64 util() const { return largo_barra - despunte - zona_muerta; }
};

struct Pieza {
    int id = 0;          // lo pone quien llama; se devuelve tal cual en el plan
    i64 largo = 0;
    int cantidad = 0;
};

struct Colocada {
    int id = 0;
    i64 largo = 0;
    i64 inicio = 0;      // medido desde la punta de la barra (incluye el despunte)
    i64 fin = 0;
};

struct BarraPlan {
    std::vector<Colocada> piezas;
    i64 ocupado = 0;     // suma(largos) + (n-1)*separacion
    i64 libre = 0;       // util - ocupado
};

enum class Estado { completo, tiempo_agotado, cancelado };

struct Plan {
    Parametros parametros;
    std::vector<BarraPlan> barras;
    std::vector<int> no_caben;   // ids de piezas mas largas que el util: fuera del calculo
    i64 cota = 0;                // cota inferior de barras
    i64 cota_lp = 0;
    bool demostrado = false;     // barras == cota
    Estado estado = Estado::completo;
    Info info;
    i64 piezas_colocadas = 0;
    i64 largo_colocado = 0;      // suma de largos colocados
    i64 sobran_hasta() const { return (i64)barras.size() - cota; }
    double aprovechamiento() const;   // largo_colocado / (barras * largo_barra)
};

// Lanza std::invalid_argument si los parametros o las piezas no tienen sentido.
Plan calcular_perfil(const std::vector<Pieza>& piezas, const Parametros& p, int arranques = 5,
                     const Control* control = nullptr);

// Validador escrito aparte: toda solucion pasa por aqui antes de mostrarse o exportarse.
bool validar_plan(const std::vector<Pieza>& piezas, const Parametros& p, const Plan& plan,
                  std::string* motivo = nullptr);

// Barras iguales consecutivas (mismas piezas en el mismo orden): "x N".
struct Grupo {
    int primera = 0;   // indice en plan.barras
    int veces = 0;
};
std::vector<Grupo> agrupar(const Plan& plan);

// ---------------------------------------------------------------------------
// Entrada de numeros: acepta coma o punto decimal y como mucho un decimal.
// Devuelve el valor en decimas (1234,5 -> 12345). false si no es un numero valido.
bool leer_decimas(const std::string& texto, i64& decimas);

}  // namespace nt
