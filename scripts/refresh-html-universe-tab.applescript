on run argv
    if (count of argv) is not 1 then
        error "usage: osascript scripts/refresh-html-universe-tab.applescript /absolute/path/to/page.html"
    end if

    set targetPath to item 1 of argv
    set targetName to do shell script "basename " & quoted form of targetPath
    set targetUrl to "file://" & targetPath

    if application "Safari" is running then
        tell application "Safari"
            repeat with browserWindow in windows
                repeat with browserTab in tabs of browserWindow
                    set candidateUrl to URL of browserTab
                    if candidateUrl is not missing value and candidateUrl contains targetName then
                        set URL of browserTab to targetUrl
                        set current tab of browserWindow to browserTab
                        set index of browserWindow to 1
                        activate
                        return "refreshed Safari tab: " & targetName
                    end if
                end repeat
            end repeat
        end tell
    end if

    if application "Google Chrome" is running then
        tell application "Google Chrome"
            repeat with browserWindow in windows
                set tabNumber to 0
                repeat with browserTab in tabs of browserWindow
                    set tabNumber to tabNumber + 1
                    set candidateUrl to URL of browserTab
                    if candidateUrl is not missing value and candidateUrl contains targetName then
                        set URL of browserTab to targetUrl
                        set active tab index of browserWindow to tabNumber
                        set index of browserWindow to 1
                        reload browserTab
                        activate
                        return "refreshed Google Chrome tab: " & targetName
                    end if
                end repeat
            end repeat
        end tell
    end if

    tell application "Safari"
        make new document with properties {URL:targetUrl}
        activate
    end tell
    return "opened in Safari: " & targetName
end run
