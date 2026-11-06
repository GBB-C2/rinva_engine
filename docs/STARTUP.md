# Если сборка проходит, а окно не открывается

`0xc0000139` / `STATUS_ENTRYPOINT_NOT_FOUND` означает, что загрузчик Windows не нашёл импортируемую функцию в DLL. Это происходит до входа в `main`, поэтому отсутствие Rust panic или файла `native-error.txt` при такой ошибке ожидаемо. Повторная компиляция без изменения причины не помогает. Сам код ошибки не сообщает имя отсутствующей функции.

## Совместимость DPI

Ранние выпуски Windows 10 не имеют `GetDpiForWindow` (появилась в 1607) или `SetProcessDpiAwarenessContext` (в документации функции указан выпуск 1703). Прежняя Rinva импортировала обе функции обязательно, поэтому на системе без одной из них окно не могло открыться.

Теперь режим DPI задаётся встроенным манифестом: `PerMonitorV2` с переходом к `PerMonitor`, а для более ранних систем остаётся `true/pm`. `GetDpiForWindow` загружается по наличию; при её отсутствии используется `GetDpiForMonitor`, затем системный DPI. Отдельного вызова `SetProcessDpiAwarenessContext` больше нет. Функции современного DPI не должны присутствовать в таблице обязательного импорта `.exe`.

Это исправляет конкретную несовместимость ранних Windows 10. Если ошибка возникает на более новой системе, необходимо проверить имя функции/DLL, манифест и фактически загружаемые библиотеки.

## Диагностика своей сборки

Обновите исходники целиком, включая `crates/rinva-desktop/build.rs` и `app.manifest`. Из корня проекта выполните в 64-битном PowerShell:

```powershell
rustc -vV
cargo run --locked -p rinva-desktop
```

Для ошибки в каталоге InkCode используйте путь из журнала:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\tools\diagnose-startup.ps1 -Executable .\.inkcode\cargo\2\debug\rinva.exe
```

При обычном запуске Cargo замените путь на `.\target\debug\rinva.exe`. В готовой сборке скрипт находится рядом с `rinva.exe`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\diagnose-startup.ps1 -Executable .\rinva.exe
```

Скрипт выводит номер сборки Windows, наличие встроенного манифеста Common Controls v6, обязательные DPI-импорты, пути системных DLL и строки `MISSING DLL` / `MISSING ENTRY POINT`, если зависимости отсутствуют. Он читает исполняемый файл, но не запускает его, и не требует прав администратора. Параметр `ExecutionPolicy` относится только к этому процессу PowerShell и не меняет постоянную настройку системы.

Проверка загружает только системные DLL и сборки SxS, выбранные манифестом. Она не проверяет подменяющие DLL в папке IDE/проекта и отложенные импорты. Если отчёт не показывает пропущенных функций, откройте сам `rinva.exe` напрямую и запишите имя функции и DLL из системного окна ошибки. Для разбора нужны эти строки, номер сборки Windows и вывод `rustc -vV`. Не заменяйте системные DLL файлами из случайных источников.

## Что проверяется автоматически

CI запускает MSVC `debug`-сборку через Cargo в `.inkcode/cargo/2`, проверяет встроенный манифест и все обычные импорты обеих сборок. Затем выполняется полный нативный smoke-тест `debug` с принудительным DPI fallback и обычный smoke-тест `release`.

Режим проверки fallback: `rinva.exe --smoke-test smoke-output --legacy-dpi`. Флаг действует только в smoke-тесте; в обычном редакторе выбор функции автоматический. Это проверяет запасной путь на Windows-runner, но не заменяет испытание на конкретной старой сборке Windows 10 или на компьютере пользователя.

Источники API: [GetDpiForWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getdpiforwindow), [SetProcessDpiAwarenessContext](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setprocessdpiawarenesscontext), [DPI через манифест](https://learn.microsoft.com/en-us/windows/win32/hidpi/setting-the-default-dpi-awareness-for-a-process).

## Журнал нативного запуска

Каждый обычный запуск создаёт небольшой локальный журнал в `%LOCALAPPDATA%/Rinva/logs/session-*.log`: вход в программу, готовность окна, нормальное закрытие или место Rust panic. Содержимое проекта в журнал не записывается. При сбое загрузчика до `main` новый журнал не появится; для него используйте проверку DLL выше. Журналы никуда не отправляются автоматически.
