// BySoftCarpeta: crea carpetas en una base de BySoft usando la API de BySoft
// (FolderInfo.CreateSubfolder), igual que el boton "Crear nueva carpeta" del
// Part Importer. Asi la carpeta queda registrada en el indice (index.db) y BySoft
// la ve sin "Actualizar indice".
//
// Uso:
//   BySoftCarpeta.exe info                         -> muestra las rutas de las bases
//   BySoftCarpeta.exe crear <Raiz> <Ruta\Relativa> -> crea (o registra) la carpeta
//      <Raiz> = Parts | PartJobs   (nombre de la base en BySoft)
//
// Se compila en el PC con el csc.exe de .NET Framework (C# 5). Usa las DLL de
// BySoft instaladas; no las copia ni las modifica.
//
// Codigos de salida: 0 OK, 1 error, 2 uso incorrecto.

using System;
using System.Configuration;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;

internal static class Programa
{
    private static string _dirBySoft = @"C:\Program Files\Bystronic\BySoft CAM\Programmer";

    private static int Main(string[] args)
    {
        string dir = Environment.GetEnvironmentVariable("BYSOFT_DIR");
        if (!string.IsNullOrEmpty(dir))
        {
            _dirBySoft = dir;
        }
        // SQLite.Interop.dll (indice) se carga desde la carpeta de BySoft.
        Environment.SetEnvironmentVariable("PreLoadSQLite_BaseDirectory", _dirBySoft);
        AppDomain.CurrentDomain.AssemblyResolve += ResolverEnBySoft;
        try
        {
            return Ejecutar(args);
        }
        catch (Exception ex)
        {
            Console.WriteLine("ERROR: " + ex);
            return 1;
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

    // NoInlining: los tipos de BySoft se resuelven despues de registrar AssemblyResolve.
    [MethodImpl(MethodImplOptions.NoInlining)]
    private static int Ejecutar(string[] args)
    {
        if (args.Length == 0)
        {
            Uso();
            return 2;
        }
        Bystronic.BySoft.Common.Persistence.PersistenceManager pm = CrearPersistenceManager();
        string modo = args[0].ToLowerInvariant();
        if (modo == "info")
        {
            foreach (string raiz in new[] { "Parts", "PartJobs" })
            {
                Bystronic.BySoft.Common.Persistence.FolderInfo r = pm.GetRootFolder(raiz);
                Console.WriteLine(raiz + "=" + r.PhysicalPath);
            }
            return 0;
        }
        if (modo == "crear" && args.Length >= 3)
        {
            return Crear(pm, args[1], args[2]);
        }
        Uso();
        return 2;
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static int Crear(Bystronic.BySoft.Common.Persistence.PersistenceManager pm, string raiz, string rutaRelativa)
    {
        Bystronic.BySoft.Common.Persistence.FolderInfo f = pm.GetRootFolder(raiz);
        Console.WriteLine("Base " + raiz + ": " + f.PhysicalPath);
        string[] partes = rutaRelativa.Replace('\\', '/').Split(new[] { '/' }, StringSplitOptions.RemoveEmptyEntries);
        if (partes.Length == 0)
        {
            Console.WriteLine("ERROR: ruta relativa vacia");
            return 2;
        }
        foreach (string parte in partes)
        {
            string nombre = parte.Trim();
            if (f.ExistsFolder(nombre))
            {
                f = f.GetFolderFromLocalPath(nombre);
                Console.WriteLine("  ya registrada: " + f.LocalPath);
            }
            else
            {
                // Si el directorio ya existe en disco (creado con mkdir/Explorador),
                // CreateSubdirectory no falla y solo se registra en el indice.
                bool existiaEnDisco = Directory.Exists(Path.Combine(f.PhysicalPath, nombre));
                f = f.CreateSubfolder(nombre);
                Console.WriteLine((existiaEnDisco ? "  registrada en indice (ya existia en disco): " : "  creada: ") + f.LocalPath);
            }
        }
        Console.WriteLine("OK " + f.LocalPath + " -> " + f.PhysicalPath);
        return 0;
    }

    // Igual que PartImporter (Program.CreateImporter): config de la aplicacion
    // + config del usuario (%APPDATA%\Bystronic\BySoftCam\Common.config), que es
    // donde estan las rutas de red de las bases.
    [MethodImpl(MethodImplOptions.NoInlining)]
    private static Bystronic.BySoft.Common.Persistence.PersistenceManager CrearPersistenceManager()
    {
        string seccion = Bystronic.BySoft.Common.Persistence.Configuration.PersistenceManagerSection.DefaultSectionName;
        Bystronic.BySoft.Common.Persistence.Configuration.PersistenceManagerSection app =
            ConfigurationManager.GetSection(seccion) as Bystronic.BySoft.Common.Persistence.Configuration.PersistenceManagerSection;
        if (app == null)
        {
            throw new InvalidOperationException("Falta la seccion persistenceManager en BySoftCarpeta.exe.config");
        }
        string appData = (string)ObtenerTipo("SpecialFolders").GetProperty("ApplicationData", BindingFlags.Public | BindingFlags.Static).GetValue(null, null);
        string cfgUsuario = Path.Combine(appData, @"Bystronic\BySoftCam\Common.config");
        Console.WriteLine("Config usuario: " + cfgUsuario + (File.Exists(cfgUsuario) ? "" : " (no existe)"));
        MethodInfo load = ObtenerTipo("ConfigurationHelper").GetMethod("LoadConfiguration", new[] { typeof(string) });
        Configuration cfg = (Configuration)load.Invoke(null, new object[] { cfgUsuario });
        Bystronic.BySoft.Common.Persistence.Configuration.PersistenceManagerSection usr =
            (cfg == null ? null : cfg.GetSection(seccion)) as Bystronic.BySoft.Common.Persistence.Configuration.PersistenceManagerSection;
        Bystronic.BySoft.Common.Persistence.PersistenceManager pm = new Bystronic.BySoft.Common.Persistence.PersistenceManager();
        pm.Configure(app, usr);
        return pm;
    }

    // ConfigurationHelper y SpecialFolders estan en las DLL comunes de BySoft;
    // se buscan por nombre para no depender del namespace exacto al compilar.
    private static Type ObtenerTipo(string nombreCorto)
    {
        foreach (string dll in new[] { "Bystronic.BySoft.Common", "Bystronic.BySoft.Infrastructure.Interface", "Bystronic.BySoft.Infrastructure.Library" })
        {
            string f = Path.Combine(_dirBySoft, dll + ".dll");
            if (!File.Exists(f))
            {
                continue;
            }
            Type t;
            try
            {
                t = Assembly.LoadFrom(f).GetTypes().FirstOrDefault(x => x.Name == nombreCorto);
            }
            catch (ReflectionTypeLoadException ex)
            {
                t = ex.Types.FirstOrDefault(x => x != null && x.Name == nombreCorto);
            }
            if (t != null)
            {
                return t;
            }
        }
        throw new InvalidOperationException("No se encontro el tipo " + nombreCorto + " en las DLL de BySoft");
    }

    private static void Uso()
    {
        Console.WriteLine("Uso:");
        Console.WriteLine("  BySoftCarpeta.exe info");
        Console.WriteLine("  BySoftCarpeta.exe crear Parts|PartJobs \"DESARROLLO\\PROYECTO\\2 ACERO\"");
    }
}
