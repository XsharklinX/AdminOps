; Datos del instalador clásico (NSIS) que la plantilla de Tauri deja vacíos.
; Windows y los antivirus los muestran en Propiedades → Detalles: un instalador
; con editor y descripción completos inspira más confianza que uno anónimo.
VIAddVersionKey "CompanyName" "David Bonilla"
VIAddVersionKey "Comments" "Diagnóstico, optimización y soporte técnico de Windows."
VIAddVersionKey "InternalName" "AdminOps-instalador"

; Al desinstalar: el arranque del cohete de quien desinstala (los demás usuarios lo ven fallar en silencio).
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "AdminOps (cohete)"
!macroend
