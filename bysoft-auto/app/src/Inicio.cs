// Punto de entrada. Antes de tocar cualquier tipo de BySoft:
//  1) usa la configuracion de PartImporter.exe.config (rutas y redirecciones de version),
//  2) resuelve las DLL de BySoft desde su carpeta de instalacion (no se copian).

using System;
using System.IO;
using System.Reflection;
using System.Windows.Forms;

namespace AutoBySoft
{
    internal static class Inicio
    {
        private static string _dirBySoft;

        [STAThread]
        private static int Main(string[] args)
        {
            string dirApp = AppDomain.CurrentDomain.BaseDirectory;
            Config cfg = Config.Cargar(dirApp);
            _dirBySoft = cfg.DirBySoft;

            string cfgBySoft = Path.Combine(_dirBySoft, "PartImporter.exe.config");
            if (!File.Exists(cfgBySoft))
            {
                MessageBox.Show("No se encuentra BySoft en:\n" + _dirBySoft + "\n\nRevisa BYSOFT_DIR en AutoBySoft.ini.", "AutoBySoft",
                                MessageBoxButtons.OK, MessageBoxIcon.Error);
                return 1;
            }
            // Usar la configuracion de BySoft como la propia (incluye las redirecciones de
            // version de sus DLL, que solo se aplican si el .config existe al arrancar).
            string cfgPropio = AppDomain.CurrentDomain.SetupInformation.ConfigurationFile;
            if (!MismoContenido(cfgBySoft, cfgPropio))
            {
                try
                {
                    File.Copy(cfgBySoft, cfgPropio, true);
                    if (args.Length == 0 || args[0] != "--reiniciado")
                    {
                        System.Diagnostics.Process.Start(Assembly.GetExecutingAssembly().Location, "--reiniciado");
                        return 0;
                    }
                }
                catch (Exception)
                {
                    // Carpeta sin permiso de escritura: usar el .config de BySoft directamente.
                    AppDomain.CurrentDomain.SetData("APP_CONFIG_FILE", cfgBySoft);
                }
            }
            Environment.SetEnvironmentVariable("PreLoadSQLite_BaseDirectory", _dirBySoft);
            AppDomain.CurrentDomain.AssemblyResolve += ResolverEnBySoft;

            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.Run(new Ventana(cfg));
            return 0;
        }

        private static bool MismoContenido(string a, string b)
        {
            try
            {
                return File.Exists(a) && File.Exists(b) && File.ReadAllText(a) == File.ReadAllText(b);
            }
            catch (Exception)
            {
                return false;
            }
        }

        private static Assembly ResolverEnBySoft(object sender, ResolveEventArgs e)
        {
            string nombre = new AssemblyName(e.Name).Name;
            foreach (string ext in new[] { ".dll", ".exe" })
            {
                string f = Path.Combine(_dirBySoft, nombre + ext);
                if (File.Exists(f))
                {
                    return Assembly.LoadFrom(f);
                }
            }
            return null;
        }
    }
}
