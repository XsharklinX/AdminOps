; Datos del instalador clásico (NSIS) que la plantilla de Tauri deja vacíos.
; Windows y los antivirus los muestran en Propiedades → Detalles: un instalador
; con editor y descripción completos inspira más confianza que uno anónimo.
VIAddVersionKey "CompanyName" "David Bonilla"
VIAddVersionKey "Comments" "Diagnóstico, optimización y soporte técnico de Windows."
VIAddVersionKey "InternalName" "AdminOps-instalador"

; Al desinstalar: el arranque del cohete de quien desinstala (los demás usuarios lo ven fallar en silencio).
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "AdminOps (cohete)"
  ; Las copias del instalador para volver a la versión anterior (las guarda installer/ en ProgramData), las marcas
  ; de «arrancó bien» y la copia antigua apuntada en HKLM. Solo al desinstalar de verdad: en una actualización hacen falta.
  ; Nota: el instalador clásico con ventanas, si se elige «desinstalar antes», desinstala sin /UPDATE y también las borra.
  ${If} $UpdateMode <> 1
    ReadEnvStr $R9 "ProgramData"
    ${If} $R9 != ""
      ; Solo lo nuestro, archivo a archivo y sin /r: si la carpeta la hubiera creado antes un usuario (instalación
      ; solo con el instalador clásico), /r podría entrar en un enlace que pusiera dentro.
      Delete "$R9\AdminOps\instalador\AdminOps-*-Setup.exe"
      Delete "$R9\AdminOps\instalador\*.nuevo"
      Delete "$R9\AdminOps\instalador\actualizacion-fallida.txt"
      RMDir "$R9\AdminOps\instalador"
      ; En «arranque» pueden crear los usuarios: solo las marcas (archivos) y la carpeta si queda vacía. Nunca /r, que
      ; podría seguir un enlace que hubiera puesto alguien.
      Delete "$R9\AdminOps\arranque\arranque-*.ok"
      RMDir "$R9\AdminOps\arranque"
    ${EndIf}
    DeleteRegKey HKLM "SOFTWARE\AdminOps\Instaladores"
    DeleteRegKey HKLM "SOFTWARE\AdminOps\VersionAnterior"
  ${EndIf}
!macroend
