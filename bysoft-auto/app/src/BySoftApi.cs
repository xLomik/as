// Acceso a BySoft mediante sus propias DLL (Bystronic.BySoft.Common.Persistence),
// igual que lo hace el Part Importer:
//  - crear carpetas registrandolas en el indice (FolderInfo.CreateSubfolder)
//  - buscar piezas por nombre en el indice
//  - leer el catalogo de System (materiales, maquinas, reglas de corte)
//  - renombrar una pieza (modo "conservar version anterior"); el GUID no cambia
// No carga ni modifica el contenido de los objetos de BySoft.

using System;
using System.Collections.Generic;
using System.Configuration;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using Bystronic.BySoft.Common.Persistence;
using Bystronic.BySoft.Common.Persistence.Configuration;

namespace AutoBySoft
{
    public sealed class ObjetoCatalogo
    {
        public string Tipo;
        public string Nombre;
        public Guid Guid;
        public string Carpeta;
    }

    public sealed class BySoftApi
    {
        private readonly PersistenceManager _pm;
        private readonly string _dirBySoft;

        [MethodImpl(MethodImplOptions.NoInlining)]
        public BySoftApi(string dirBySoft)
        {
            _dirBySoft = dirBySoft;
            string seccion = PersistenceManagerSection.DefaultSectionName;
            PersistenceManagerSection app = ConfigurationManager.GetSection(seccion) as PersistenceManagerSection;
            if (app == null)
            {
                throw new InvalidOperationException("No se pudo leer la configuracion de BySoft (seccion persistenceManager de PartImporter.exe.config).");
            }
            string appData = (string)ObtenerTipo("SpecialFolders").GetProperty("ApplicationData", BindingFlags.Public | BindingFlags.Static).GetValue(null, null);
            ConfigUsuario = Path.Combine(appData, @"Bystronic\BySoftCam\Common.config");
            MethodInfo load = ObtenerTipo("ConfigurationHelper").GetMethod("LoadConfiguration", new[] { typeof(string) });
            Configuration cfg = (Configuration)load.Invoke(null, new object[] { ConfigUsuario });
            PersistenceManagerSection usr = (cfg == null ? null : cfg.GetSection(seccion)) as PersistenceManagerSection;
            _pm = new PersistenceManager();
            _pm.Configure(app, usr);
        }

        public string ConfigUsuario { get; private set; }

        public string RutaRaiz(string raiz)
        {
            return _pm.GetRootFolder(raiz).PhysicalPath;
        }

        // Crea (o registra en el indice, si ya existia en disco) cada nivel de la ruta.
        // Devuelve el texto de lo que hizo con cada nivel.
        public List<string> CrearCarpeta(string raiz, string rutaRelativa)
        {
            List<string> log = new List<string>();
            FolderInfo f = _pm.GetRootFolder(raiz);
            foreach (string parte in Partes(rutaRelativa))
            {
                if (f.ExistsFolder(parte))
                {
                    f = f.GetFolderFromLocalPath(parte);
                    log.Add("ya registrada " + f.LocalPath);
                }
                else
                {
                    bool enDisco = Directory.Exists(Path.Combine(f.PhysicalPath, parte));
                    f = f.CreateSubfolder(parte);
                    log.Add((enDisco ? "registrada en indice (ya existia en disco) " : "creada ") + f.LocalPath);
                }
            }
            return log;
        }

        // Ruta de la carpeta en formato del .pis: "/DESARROLLO/PROYECTO/SUB/"
        public static string RutaLocal(string rutaRelativa)
        {
            return "/" + string.Join("/", Partes(rutaRelativa)) + "/";
        }

        public static string[] Partes(string rutaRelativa)
        {
            return (rutaRelativa ?? "").Replace('\\', '/').Split(new[] { '/' }, StringSplitOptions.RemoveEmptyEntries)
                .Select(p => p.Trim()).Where(p => p.Length > 0).ToArray();
        }

        // Piezas con ese nombre exacto en toda la base Parts. Devuelve la carpeta de cada una.
        public List<string> BuscarPieza(string nombre)
        {
            SearchObjectCriteria c = new SearchObjectCriteria(true);   // nombre exacto, sin listas
            c.Name = nombre;
            c.SearchSubFolders = true;
            List<string> r = new List<string>();
            foreach (ObjectInfo o in _pm.GetRootFolder("Parts").SearchObjects(c))
            {
                r.Add(o.Folder == null ? "(sin carpeta)" : o.Folder.LocalPath);
            }
            return r;
        }

        // Renombra (archivo .box, archivos asociados e indice) la pieza 'nombre' de la carpeta indicada.
        // Usa ObjectInfo.Rename de BySoft: respeta los bloqueos y conserva el GUID.
        public void RenombrarPieza(string nombre, string carpetaLocal, string nuevoNombre)
        {
            SearchObjectCriteria c = new SearchObjectCriteria(true);
            c.Name = nombre;
            c.SearchSubFolders = true;
            ObjectInfo o = _pm.GetRootFolder("Parts").SearchObjects(c)
                .FirstOrDefault(x => x.Folder != null && string.Equals(x.Folder.LocalPath, carpetaLocal, StringComparison.OrdinalIgnoreCase));
            if (o == null)
            {
                throw new InvalidOperationException("No se encontro la pieza " + nombre + " en " + carpetaLocal);
            }
            o.Rename(nuevoNombre);
        }

        public List<ObjetoCatalogo> Catalogo(string raiz)
        {
            SearchObjectCriteria c = new SearchObjectCriteria();
            c.SearchSubFolders = true;
            List<ObjetoCatalogo> l = new List<ObjetoCatalogo>();
            foreach (ObjectInfo o in _pm.GetRootFolder(raiz).SearchObjects(c))
            {
                ObjetoCatalogo x = new ObjetoCatalogo();
                x.Tipo = o.TypeName;
                x.Nombre = o.Name;
                x.Guid = o.Guid;
                x.Carpeta = o.Folder == null ? "" : o.Folder.LocalPath;
                l.Add(x);
            }
            return l;
        }

        // ConfigurationHelper y SpecialFolders estan en las DLL comunes de BySoft.
        private Type ObtenerTipo(string nombreCorto)
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
            throw new InvalidOperationException("No se encontro " + nombreCorto + " en las DLL de BySoft");
        }
    }
}
