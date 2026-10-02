# Automatizacion BySoft CAM: importar piezas + crear nesteo

Estado: **fase de investigacion**. Todavia no hay automatizacion.

## Paso 1: diagnostico (solo lectura)

1. Copia **solo** `Ejecutar-diagnostico.bat` al PC que tiene BySoft CAM instalado.
   Es un archivo unico: lleva `diagnostico-bysoft.ps1` incrustado al final.
2. Abre BySoft CAM (asi el script detecta la ruta de instalacion y los puertos de ApiService).
3. Doble clic en `Ejecutar-diagnostico.bat`.
4. Se genera `diagnostico-bysoft.txt` en el Escritorio. **Revisalo** y enviamelo.

El script no ejecuta ningun programa de BySoft y no cambia nada.
Las contrasenas que aparezcan en los archivos de configuracion salen ocultas como `****`.

## Mantenimiento

Si modificas `diagnostico-bysoft.ps1`, vuelve a generar el `.bat` con `python generar_bat.py`.
