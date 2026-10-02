# Arma AutoBySoft.zip con el programa, la configuracion, el formato y el codigo fuente.
import zipfile
from pathlib import Path
app = Path(__file__).resolve().parent.parent
raiz = "AutoBySoft/"
archivos = {
    "AutoBySoft.exe": app / "dist" / "AutoBySoft.exe",
    "AutoBySoft.ini": app / "config" / "AutoBySoft.ini",
    "materiales.txt": app / "config" / "materiales.txt",
    "tecnologia.txt": app / "config" / "tecnologia.txt",
    "plantilla.pis": app / "config" / "plantilla.pis",
    "LEEME.txt": app / "LEEME.txt",
    "Compilar-en-este-PC.bat": app / "Compilar-en-este-PC.bat",
    "FORMATO_CANTIDADES.xlsx": app.parent / "formato" / "FORMATO_CANTIDADES.xlsx",
}
with zipfile.ZipFile(app / "dist" / "AutoBySoft.zip", "w", zipfile.ZIP_DEFLATED) as z:
    for nombre, ruta in archivos.items():
        z.write(ruta, raiz + nombre)
    for cs in sorted((app / "src").glob("*.cs")):
        z.write(cs, raiz + "src/" + cs.name)
print("OK", app / "dist" / "AutoBySoft.zip")
