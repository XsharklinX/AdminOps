# Se ejecuta DENTRO de Windows Sandbox (LogonCommand). No usar en un PC real.
$p = Start-Process C:\work\AdminOps.exe -ArgumentList '--roundtrip', 'C:\work\roundtrip.json' -Verb RunAs -Wait -PassThru
Set-Content C:\work\done.txt $p.ExitCode
shutdown.exe /s /t 5
