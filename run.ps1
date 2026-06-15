$env:RUST_BACKTRACE = "1"
$log = "$env:TEMP\apex-native-stderr.log"
Write-Host "Launching apex-native (backtrace ON, stderr -> $log)"
& ".\src-tauri\target\release\apex-native.exe" 2>$log
