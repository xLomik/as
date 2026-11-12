//! Iniciar Lince al encender el equipo: una tarea programada que lo abre
//! junto al reloj al iniciar sesión, con permisos de administrador y sin
//! preguntar (UAC), para que lea los discos desde el primer momento.
//! Aquí se arma la tarea; `schtasks` se llama desde `win`.

use std::path::{Path, PathBuf};

/// Nombre de la tarea en el Programador de tareas.
pub const TASK_NAME: &str = "Lince";
/// Opción con la que la tarea abre Lince: escondido, junto al reloj.
pub const STARTUP_FLAG: &str = "--inicio";

/// La tarea en el formato XML de `schtasks /Create /XML`. Se escribe a mano
/// porque `schtasks /SC ONLOGON` no deja quitar dos valores por defecto que
/// aquí estorban: no arrancar con batería y matar el proceso a las 72 horas.
pub fn task_xml(exe: &Path, user: &str) -> String {
    let exe = exe.to_string_lossy();
    // La carpeta del programa (rutas de Windows también fuera de Windows).
    let dir = escape(exe.rfind(['\\', '/']).map_or("", |i| &exe[..i]));
    let exe = escape(&exe);
    let user = escape(user);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Abre Lince junto al reloj al iniciar sesión, para que el índice esté listo.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>{STARTUP_FLAG}</Arguments>
      <WorkingDirectory>{dir}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#
    )
}

/// Programa que abre una tarea ya registrada (lo que dice `<Command>`).
pub fn task_exe(xml: &str) -> Option<PathBuf> {
    let start = xml.find("<Command>")? + "<Command>".len();
    let end = start + xml[start..].find("</Command>")?;
    Some(PathBuf::from(unescape(xml[start..end].trim())))
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_task_runs_lince_hidden_for_this_user_without_limits() {
        let exe = Path::new(r"C:\Programas & Datos\Lince\Lince.exe");
        let xml = task_xml(exe, r"OFICINA\Camilo");
        assert!(xml.contains(r"<Command>C:\Programas &amp; Datos\Lince\Lince.exe</Command>"));
        assert!(xml.contains("<Arguments>--inicio</Arguments>"));
        assert!(
            xml.contains(r"<WorkingDirectory>C:\Programas &amp; Datos\Lince</WorkingDirectory>")
        );
        assert_eq!(xml.matches(r"<UserId>OFICINA\Camilo</UserId>").count(), 2);
        // Lo que hace falta para que funcione siempre:
        for needed in [
            "<LogonTrigger>",
            "<RunLevel>HighestAvailable</RunLevel>",
            "<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>",
            "<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>",
            "<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>",
            "<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>",
        ] {
            assert!(xml.contains(needed), "falta {needed}");
        }
        assert_eq!(task_exe(&xml).as_deref(), Some(exe));
    }

    /// Lo que devuelve `schtasks /Query /XML` (con sangrías propias).
    #[test]
    fn reads_the_program_of_a_registered_task() {
        let xml = "<Actions Context=\"Author\">\r\n    <Exec>\r\n      \
                   <Command>D:\\Lince\\Lince.exe</Command>\r\n";
        assert_eq!(task_exe(xml), Some(PathBuf::from(r"D:\Lince\Lince.exe")));
        assert_eq!(task_exe("<Task></Task>"), None);
    }
}
