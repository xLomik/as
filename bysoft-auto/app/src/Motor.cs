// Orquestador: 1) Revisar (no toca BySoft salvo lecturas del indice)
//              2) Ejecutar (crea carpetas, genera .pis, importa DXF, genera Excel)

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;

namespace AutoBySoft
{
    public sealed class Config
    {
        public string DirBySoft = @"C:\Program Files\Bystronic\BySoft CAM\Programmer";
        public string DirPar = @"C:\BystronicData\BySoftCam\8109C_BySprint_Fiber_4020_6000_BIMO2_POWERCUT\Parametros_europa";
        public string Maquina = "BYSPRINT FIBER 4020";
        public string PartsEsperada = "";
        public string Listado = "";
        public string DirApp;

        public static Config Cargar(string dirApp)
        {
            Config c = new Config();
            c.DirApp = dirApp;
            string ini = Path.Combine(dirApp, "AutoBySoft.ini");
            if (!File.Exists(ini))
            {
                return c;
            }
            foreach (string l in File.ReadAllLines(ini, Encoding.UTF8))
            {
                string t = l.Trim();
                int i = t.IndexOf('=');
                if (t.StartsWith("#") || i <= 0)
                {
                    continue;
                }
                string k = t.Substring(0, i).Trim().ToUpperInvariant();
                string v = t.Substring(i + 1).Trim();
                if (k == "BYSOFT_DIR") c.DirBySoft = v;
                else if (k == "PAR_DIR") c.DirPar = v;
                else if (k == "MAQUINA") c.Maquina = v;
                else if (k == "PARTS_ESPERADA") c.PartsEsperada = v;
                else if (k == "LISTADO") c.Listado = v;
            }
            return c;
        }

        public string Archivo(string nombre)
        {
            return Path.Combine(DirApp, nombre);
        }
    }

    // Que hacer si una pieza del pedido ya existe (una vez) en BySoft.
    public enum ModoExistentes
    {
        Detener,        // error: no se toca nada
        Usar,           // se usa la existente, no se importa
        Actualizar      // se vuelve a importar el DXF y se sobrescribe en SU carpeta actual
    }

    public sealed class Entrada
    {
        public string CarpetaPedido;
        public string ExcelCantidades;
        public string Prefijo;              // LP / LD
        public string DestinoBase;          // DESARROLLO\150. Proyecto X
        public int Consecutivo;
        public ModoExistentes Existentes = ModoExistentes.Detener;
    }

    public sealed class Plan
    {
        public List<Programa> Programas = new List<Programa>();
        public List<string> Errores = new List<string>();
        public List<string> Avisos = new List<string>();
        public Guid Maquina;
        public Dictionary<string, Guid> Materiales = new Dictionary<string, Guid>(StringComparer.OrdinalIgnoreCase);
        public Dictionary<string, Guid> Reglas = new Dictionary<string, Guid>(StringComparer.OrdinalIgnoreCase);
        public bool Ok { get { return Errores.Count == 0 && Programas.Count > 0; } }
    }

    public sealed class Motor
    {
        private readonly Config _cfg;
        private readonly Action<string> _log;
        private BySoftApi _api;

        public Motor(Config cfg, Action<string> log)
        {
            _cfg = cfg;
            _log = log;
        }

        private BySoftApi Api
        {
            get
            {
                if (_api == null)
                {
                    _api = new BySoftApi(_cfg.DirBySoft);
                }
                return _api;
            }
        }

        // ================================================================ REVISAR
        public Plan Revisar(Entrada e)
        {
            Plan plan = new Plan();
            _log("=== REVISION ===");
            if (!Directory.Exists(e.CarpetaPedido)) plan.Errores.Add("No existe la carpeta del pedido.");
            if (!File.Exists(e.ExcelCantidades)) plan.Errores.Add("No existe el Excel de cantidades.");
            if (BySoftApi.Partes(e.DestinoBase).Length == 0) plan.Errores.Add("Falta la carpeta destino en BySoft (ej. DESARROLLO\\150. Proyecto X).");
            if (e.Consecutivo <= 0) plan.Errores.Add("Falta el consecutivo del primer programa.");
            foreach (string f in new[] { "materiales.txt", "tecnologia.txt", "plantilla.pis" })
            {
                if (!File.Exists(_cfg.Archivo(f))) plan.Errores.Add("Falta el archivo de configuracion " + f + " junto al programa.");
            }
            if (!Directory.Exists(_cfg.DirPar)) plan.Errores.Add("No existe la carpeta de parametros: " + _cfg.DirPar);
            if (plan.Errores.Count > 0)
            {
                return plan;
            }

            List<KeyValuePair<string, string>> alias = Reglas.LeerAlias(_cfg.Archivo("materiales.txt"));
            Dictionary<string, Familia> familias = Reglas.LeerFamilias(_cfg.Archivo("tecnologia.txt"));
            List<ParInfo> pars = Reglas.LeerPars(_cfg.DirPar);
            _log("Parametros .PAR leidos: " + pars.Count);

            // --- Excel de cantidades
            List<string[]> filas;
            try
            {
                filas = XlsxLector.Leer(e.ExcelCantidades, "Cantidades");
            }
            catch (InvalidDataException)
            {
                filas = XlsxLector.Leer(e.ExcelCantidades, null);
            }
            List<Pieza> piezas = Reglas.LeerCantidades(filas, plan.Errores, plan.Avisos);
            _log("Referencias en el Excel: " + piezas.Count);

            // --- Subcarpetas del pedido = programas
            foreach (string suelto in Dxfs(e.CarpetaPedido))
            {
                plan.Errores.Add("DXF fuera de subcarpeta (muevelo a la subcarpeta de su material): " + Path.GetFileName(suelto));
            }
            Dictionary<string, Programa> dxfAPrograma = new Dictionary<string, Programa>(StringComparer.OrdinalIgnoreCase);
            Dictionary<string, string> dxfRuta = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (string dir in Directory.GetDirectories(e.CarpetaPedido).OrderBy(d => d, StringComparer.OrdinalIgnoreCase))
            {
                string sub = Path.GetFileName(dir);
                if (sub.StartsWith("_"))
                {
                    continue;   // carpetas de salida de esta herramienta
                }
                string[] dxfs = Dxfs(dir);
                if (dxfs.Length == 0)
                {
                    plan.Avisos.Add("Subcarpeta sin DXF, se ignora: " + sub);
                    continue;
                }
                Programa p = new Programa();
                p.Subcarpeta = sub;
                p.RutaSubcarpeta = dir;
                string mat;
                double esp;
                if (!Reglas.InterpretarSubcarpeta(sub, out mat, out esp))
                {
                    plan.Errores.Add("No se entiende el espesor de la subcarpeta '" + sub + "'. Usa el formato: MATERIAL Esp=4.5mm");
                    continue;
                }
                p.MaterialTexto = mat;
                p.EspesorReal = esp;
                string fam = Reglas.FamiliaDe(mat, alias);
                if (mat.Length == 0)
                {
                    fam = "ACERO";
                    p.MaterialAsumido = true;
                    plan.Avisos.Add("La subcarpeta '" + sub + "' no indica material: se asume ACERO.");
                }
                if (fam == null)
                {
                    plan.Errores.Add("Material '" + mat + "' (subcarpeta '" + sub + "') no esta en materiales.txt. Agregalo con su familia.");
                    continue;
                }
                Familia f;
                if (!familias.TryGetValue(fam, out f))
                {
                    plan.Errores.Add("La familia " + fam + " no esta definida en tecnologia.txt.");
                    continue;
                }
                p.Familia = f;
                p.Par = Reglas.ElegirPar(f, esp, pars);
                if (p.Par == null)
                {
                    plan.Errores.Add("No hay parametro .PAR de " + fam + " para " + Reglas.Num(esp) + " mm o mas (subcarpeta '" + sub + "').");
                    continue;
                }
                p.MaterialBySoft = f.MaterialParABySoft[p.Par.Material];
                p.DestinoRelativo = string.Join("\\", BySoftApi.Partes(e.DestinoBase)) + "\\" + sub;
                foreach (string d in dxfs)
                {
                    string nombre = Path.GetFileNameWithoutExtension(d);
                    Programa otro;
                    if (dxfAPrograma.TryGetValue(nombre, out otro))
                    {
                        plan.Errores.Add("El DXF " + nombre + " esta en dos subcarpetas: '" + otro.Subcarpeta + "' y '" + sub + "'.");
                        continue;
                    }
                    dxfAPrograma[nombre] = p;
                    dxfRuta[nombre] = d;
                }
                plan.Programas.Add(p);
            }

            // --- Cada referencia del Excel con su DXF
            HashSet<string> usados = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            foreach (Pieza pz in piezas)
            {
                Programa p;
                if (!dxfAPrograma.TryGetValue(pz.Referencia, out p))
                {
                    plan.Errores.Add("No hay DXF para la referencia " + pz.Referencia + " en ninguna subcarpeta.");
                    continue;
                }
                pz.RutaDxf = dxfRuta[pz.Referencia];
                pz.Referencia = Path.GetFileNameWithoutExtension(pz.RutaDxf);   // mayusculas como el archivo
                p.Piezas.Add(pz);
                usados.Add(pz.Referencia);
            }
            foreach (KeyValuePair<string, Programa> kv in dxfAPrograma)
            {
                if (!usados.Contains(kv.Key))
                {
                    kv.Value.DxfSinCantidad.Add(kv.Key);
                    plan.Avisos.Add("DXF sin cantidad en el Excel (no se importa): " + kv.Value.Subcarpeta + "\\" + kv.Key);
                }
            }
            plan.Programas.RemoveAll(p =>
            {
                if (p.Piezas.Count == 0)
                {
                    plan.Avisos.Add("La subcarpeta '" + p.Subcarpeta + "' no tiene piezas con cantidad: no genera programa.");
                    return true;
                }
                return false;
            });
            if (plan.Errores.Count > 0)
            {
                return plan;
            }

            // --- BySoft: base, catalogo y nombres repetidos
            _log("Consultando BySoft...");
            try
            {
                string parts = Api.RutaRaiz("Parts");
                _log("Base de piezas: " + parts);
                if (_cfg.PartsEsperada.Length > 0 && !string.Equals(parts.TrimEnd('\\'), _cfg.PartsEsperada.TrimEnd('\\'), StringComparison.OrdinalIgnoreCase))
                {
                    plan.Errores.Add("BySoft usa otra base de piezas (" + parts + "), distinta de PARTS_ESPERADA en AutoBySoft.ini.");
                    return plan;
                }
                List<ObjetoCatalogo> cat = Api.Catalogo("System");
                ObjetoCatalogo maq = cat.FirstOrDefault(o => o.Tipo == "CuttingMachine" && string.Equals(o.Nombre, _cfg.Maquina, StringComparison.OrdinalIgnoreCase));
                if (maq == null)
                {
                    plan.Errores.Add("No se encontro la maquina '" + _cfg.Maquina + "' en BySoft.");
                }
                else
                {
                    plan.Maquina = maq.Guid;
                }
                foreach (Programa p in plan.Programas)
                {
                    ObjetoCatalogo m = cat.FirstOrDefault(o => o.Tipo == "Material" && string.Equals(o.Nombre, p.MaterialBySoft, StringComparison.OrdinalIgnoreCase));
                    if (m == null) plan.Errores.Add("No existe el material '" + p.MaterialBySoft + "' en BySoft.");
                    else plan.Materiales[p.MaterialBySoft] = m.Guid;
                    ObjetoCatalogo r = cat.FirstOrDefault(o => o.Tipo == "CuttingRuleSet" && string.Equals(o.Nombre, p.Familia.ReglaDeCorte, StringComparison.OrdinalIgnoreCase));
                    if (r == null) plan.Errores.Add("No existe el asistente de corte '" + p.Familia.ReglaDeCorte + "' en BySoft.");
                    else plan.Reglas[p.Familia.ReglaDeCorte] = r.Guid;
                }
                int total = plan.Programas.Sum(p => p.Piezas.Count), n = 0;
                foreach (Programa p in plan.Programas)
                {
                    foreach (Pieza pz in p.Piezas)
                    {
                        n++;
                        if (n % 20 == 0) _log("  revisando nombres en BySoft " + n + "/" + total);
                        pz.UbicacionesEnBySoft = Api.BuscarPieza(pz.Referencia);
                        if (pz.UbicacionesEnBySoft.Count > 1)
                        {
                            plan.Errores.Add("La pieza " + pz.Referencia + " ya existe " + pz.UbicacionesEnBySoft.Count + " veces en BySoft (" +
                                             string.Join(", ", pz.UbicacionesEnBySoft) + "). El Part Nester no la podra cargar; deja solo una.");
                        }
                        else if (pz.UbicacionesEnBySoft.Count == 1)
                        {
                            string donde = pz.UbicacionesEnBySoft[0];
                            if (e.Existentes == ModoExistentes.Usar)
                            {
                                pz.Importar = false;
                                plan.Avisos.Add("La pieza " + pz.Referencia + " ya existe en " + donde + ": se usa la existente (no se importa).");
                            }
                            else if (e.Existentes == ModoExistentes.Actualizar)
                            {
                                if (!donde.StartsWith("/"))
                                {
                                    plan.Errores.Add("La pieza " + pz.Referencia + " existe en BySoft pero sin carpeta en el indice; no se puede actualizar. Borrala desde BySoft.");
                                }
                                else
                                {
                                    pz.Sobrescribir = true;
                                    pz.CarpetaLocal = donde;
                                    plan.Avisos.Add("La pieza " + pz.Referencia + " ya existe en " + donde + ": se ACTUALIZA ahi con el DXF nuevo.");
                                }
                            }
                            else
                            {
                                plan.Errores.Add("La pieza " + pz.Referencia + " ya existe en BySoft (" + donde +
                                                 "). Renombra el DXF o elige 'Usar la existente' o 'Actualizar'.");
                            }
                        }
                    }
                }
            }
            catch (Exception ex)
            {
                plan.Errores.Add("Error consultando BySoft: " + ex.Message);
                _log(ex.ToString());
            }

            // --- Nombres de programa
            int cons = e.Consecutivo;
            DateTime hoy = DateTime.Now;
            foreach (Programa p in plan.Programas)
            {
                p.Consecutivo = cons++;
                p.Nombre = Reglas.NombrePrograma(e.Prefijo, hoy, p.Consecutivo);
            }
            return plan;
        }

        public string Resumen(Plan plan)
        {
            StringBuilder sb = new StringBuilder();
            foreach (Programa p in plan.Programas)
            {
                sb.AppendLine(p.Nombre + "  |  " + p.Subcarpeta + "  |  " + p.Piezas.Count + " piezas, " + p.Piezas.Sum(x => x.Cantidad) + " unidades");
                sb.AppendLine("      material " + (p.MaterialTexto.Length > 0 ? p.MaterialTexto : "(no indicado)") + " " + Reglas.Num(p.EspesorReal) + " mm  ->  BySoft "
                              + p.MaterialBySoft + " " + Reglas.Num(p.Par.Espesor) + " mm, " + p.Familia.ReglaDeCorte + ", " + p.Par.Archivo);
                sb.AppendLine("      destino " + p.DestinoRelativo);
            }
            if (plan.Avisos.Count > 0)
            {
                sb.AppendLine();
                sb.AppendLine("AVISOS:");
                foreach (string a in plan.Avisos) sb.AppendLine("  - " + a);
            }
            if (plan.Errores.Count > 0)
            {
                sb.AppendLine();
                sb.AppendLine("ERRORES (hay que corregirlos antes de ejecutar):");
                foreach (string er in plan.Errores) sb.AppendLine("  * " + er);
            }
            return sb.ToString();
        }

        // ================================================================ EJECUTAR
        public bool Ejecutar(Plan plan, Entrada e, out string carpetaSalida)
        {
            string sello = DateTime.Now.ToString("yyyyMMdd_HHmmss", CultureInfo.InvariantCulture);
            carpetaSalida = Path.Combine(e.CarpetaPedido, "_AutoBySoft_" + sello);
            Directory.CreateDirectory(carpetaSalida);
            string staging = Path.Combine(Path.GetTempPath(), "AutoBySoft_" + sello);
            string plantilla = File.ReadAllText(_cfg.Archivo("plantilla.pis"), Encoding.UTF8);
            string importer = Path.Combine(_cfg.DirBySoft, "PartImporter.exe");
            bool todoOk = true;
            List<Programa> listos = new List<Programa>();

            _log("=== EJECUCION === salida: " + carpetaSalida);
            foreach (Programa p in plan.Programas)
            {
                _log("");
                _log("--- " + p.Nombre + " (" + p.Subcarpeta + ")");
                try
                {
                    foreach (string l in Api.CrearCarpeta("Parts", p.DestinoRelativo)) _log("  Parts: " + l);
                    foreach (string l in Api.CrearCarpeta("PartJobs", p.DestinoRelativo)) _log("  PartJobs: " + l);

                    // Piezas nuevas -> carpeta del programa; piezas a actualizar -> su carpeta actual.
                    string destinoPrograma = BySoftApi.RutaLocal(p.DestinoRelativo);
                    foreach (Pieza pz in p.Piezas)
                    {
                        if (pz.Importar && string.IsNullOrEmpty(pz.CarpetaLocal))
                        {
                            pz.CarpetaLocal = destinoPrograma;
                        }
                    }
                    int grupo = 0;
                    foreach (IGrouping<string, Pieza> g in p.Piezas.Where(x => x.Importar)
                                 .GroupBy(x => x.CarpetaLocal, StringComparer.OrdinalIgnoreCase))
                    {
                        grupo++;
                        bool sobrescribir = g.Any(x => x.Sobrescribir);
                        string sufijo = grupo == 1 ? "" : "_" + grupo;
                        string pis = Pis.Generar(plantilla, plan.Materiales[p.MaterialBySoft], p.Par.Espesor, plan.Maquina,
                                                 plan.Reglas[p.Familia.ReglaDeCorte], p.Par.Archivo, g.Key, sobrescribir);
                        string rutaPis = Path.Combine(carpetaSalida, p.Nombre + sufijo + ".pis");
                        File.WriteAllText(rutaPis, pis, new UTF8Encoding(false));
                        string dirStaging = Path.Combine(staging, p.Nombre + sufijo);
                        Directory.CreateDirectory(dirStaging);
                        foreach (Pieza pz in g)
                        {
                            File.Copy(pz.RutaDxf, Path.Combine(dirStaging, Path.GetFileName(pz.RutaDxf)), true);
                        }
                        string rutaLog = Path.Combine(carpetaSalida, p.Nombre + sufijo + "_importacion.log");
                        _log("  Importando " + g.Count() + " DXF en " + g.Key + (sobrescribir ? " (actualizando existentes)" : "") + "...");
                        int cod = EjecutarImporter(importer, rutaPis, dirStaging, rutaLog);
                        _log("  Part Importer termino con codigo " + cod + (cod == 0 ? " (OK)" : ""));
                        if (File.Exists(rutaLog))
                        {
                            foreach (string l in File.ReadAllLines(rutaLog, Encoding.Default))
                            {
                                if (l.IndexOf("ERROR", StringComparison.OrdinalIgnoreCase) >= 0 ||
                                    l.IndexOf("no se tiene en cuenta", StringComparison.OrdinalIgnoreCase) >= 0 ||
                                    l.IndexOf("No se ha encontrado", StringComparison.OrdinalIgnoreCase) >= 0)
                                {
                                    _log("    log: " + l);
                                }
                            }
                        }
                        if (cod != 0)
                        {
                            _log("  ATENCION: el Part Importer reporto errores (ver " + Path.GetFileName(rutaLog) + ").");
                        }
                    }

                    // Verificar que cada pieza exista UNA vez en BySoft antes de generar el Excel:
                    // si el Excel nombra una pieza inexistente, el Part Nester crea una pieza VACIA.
                    List<string> faltan = new List<string>();
                    foreach (Pieza pz in p.Piezas)
                    {
                        List<string> u = Api.BuscarPieza(pz.Referencia);
                        if (u.Count != 1)
                        {
                            faltan.Add(pz.Referencia + " (encontrada " + u.Count + " veces)");
                        }
                        else if (pz.Importar && !string.Equals(u[0], pz.CarpetaLocal, StringComparison.OrdinalIgnoreCase))
                        {
                            faltan.Add(pz.Referencia + " (quedo en " + u[0] + ")");
                        }
                    }
                    if (faltan.Count > 0)
                    {
                        todoOk = false;
                        _log("  ERROR: piezas que no quedaron bien en BySoft; NO se genera el Excel de este programa:");
                        foreach (string f in faltan) _log("    - " + f);
                        continue;
                    }

                    List<string[]> filas = new List<string[]>();
                    foreach (Pieza pz in p.Piezas)
                    {
                        // A nombre | B cantidad (texto) | C info pedido | D info1 | E info2 | F info3 ; sin encabezado
                        filas.Add(new[] { pz.Referencia, pz.Cantidad.ToString(CultureInfo.InvariantCulture), p.Nombre, pz.Observacion, "", "" });
                    }
                    string rutaXlsx = Path.Combine(carpetaSalida, p.Nombre + "_" + Seguro(p.Subcarpeta) + ".xlsx");
                    XlsxEscritor.Escribir(rutaXlsx, "Piezas", filas);
                    _log("  OK. Excel para el Part Nester: " + Path.GetFileName(rutaXlsx));
                    listos.Add(p);
                }
                catch (Exception ex)
                {
                    todoOk = false;
                    _log("  ERROR: " + ex.Message);
                    _log(ex.ToString());
                }
            }

            EscribirFilasListado(listos, e, Path.Combine(carpetaSalida, "FILAS_PARA_LISTADO.xlsx"));
            try { if (Directory.Exists(staging)) Directory.Delete(staging, true); } catch { }
            _log("");
            _log(todoOk ? "=== TERMINADO SIN ERRORES ===" : "=== TERMINADO CON ERRORES: revisa el registro ===");
            return todoOk;
        }

        private static int EjecutarImporter(string exe, string pis, string carpeta, string log)
        {
            ProcessStartInfo psi = new ProcessStartInfo();
            psi.FileName = exe;
            psi.Arguments = "\"-s=" + pis + "\" \"-dir=" + carpeta + "\" \"-log=" + log + "\"";
            psi.UseShellExecute = false;
            psi.CreateNoWindow = true;
            using (Process pr = Process.Start(psi))
            {
                pr.WaitForExit();
                return pr.ExitCode;
            }
        }

        private void EscribirFilasListado(List<Programa> programas, Entrada e, string ruta)
        {
            if (programas.Count == 0)
            {
                return;
            }
            bool lp = e.Prefijo == "LP";
            string proyecto = BySoftApi.Partes(e.DestinoBase).Last();
            string referencia = Path.GetFileName(e.CarpetaPedido.TrimEnd('\\'));
            DateTime hoy = DateTime.Now;
            List<string[]> filas = new List<string[]>();
            filas.Add(lp
                ? new[] { "FECHA CREACION", "CODIGO", "MES", "AÑO", "CONSECUTIVO", "REVISION", "NOMBRE", "REFERENCIA", "ESP", "MATERIAL", "PROY/CLIENTE", "PROGRAMA" }
                : new[] { "FECHA CREACION", "CODIGO", "MES", "AÑO", "CONSECUTIVO", "NOMBRE", "REFERENCIA", "ESP", "MATERIAL", "PROY/CLIENTE", "PROGRAMA" });
            foreach (Programa p in programas)
            {
                string fecha = hoy.ToString("dd/MM/yyyy", CultureInfo.InvariantCulture);
                string mes = hoy.ToString("MM", CultureInfo.InvariantCulture), anio = hoy.ToString("yy", CultureInfo.InvariantCulture);
                string mat = p.MaterialTexto.Length > 0 ? p.MaterialTexto : "";
                filas.Add(lp
                    ? new[] { fecha, e.Prefijo, mes, anio, p.Consecutivo.ToString(CultureInfo.InvariantCulture), "R0", proyecto, referencia, Reglas.Num(p.EspesorReal), mat, proyecto, p.Nombre }
                    : new[] { fecha, e.Prefijo, mes, anio, p.Consecutivo.ToString(CultureInfo.InvariantCulture), proyecto, referencia, Reglas.Num(p.EspesorReal), mat, proyecto, p.Nombre });
            }
            XlsxEscritor.Escribir(ruta, "Filas", filas);
            _log("Filas para el listado de programas: " + Path.GetFileName(ruta));
        }

        // *.dxf sin distinguir mayusculas (D_R0.DXF, d_r0.dxf...).
        private static string[] Dxfs(string carpeta)
        {
            return Directory.GetFiles(carpeta)
                .Where(f => string.Equals(Path.GetExtension(f), ".dxf", StringComparison.OrdinalIgnoreCase))
                .OrderBy(f => f, StringComparer.OrdinalIgnoreCase).ToArray();
        }

        private static string Seguro(string s)
        {
            foreach (char c in Path.GetInvalidFileNameChars())
            {
                s = s.Replace(c, '_');
            }
            return s;
        }
    }
}
