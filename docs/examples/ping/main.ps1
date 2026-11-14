# Programme de l'addon « Ping » pour HubForge.
# HubForge écrit une ligne JSON par message sur l'entrée standard ;
# on lui répond par des lignes JSON sur la sortie standard.

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false

function Send($obj) {
    [Console]::Out.WriteLine(($obj | ConvertTo-Json -Compress))
    [Console]::Out.Flush()
}

$target = '1.1.1.1'
$every = 5
$key = $null
$running = $true
$next = Get-Date
$ping = New-Object System.Net.NetworkInformation.Ping
# Lecture de l'entrée standard sans bloquer la boucle (Console.In ne le permet pas).
$stdin = New-Object System.IO.StreamReader([Console]::OpenStandardInput(), (New-Object System.Text.UTF8Encoding $false))
$pending = $stdin.ReadLineAsync()

function Apply($s) {
    if ($s.host) { $script:target = [string]$s.host }
    if ($s.every) { $script:every = [Math]::Max(1, [int]$s.every) }
    $script:next = Get-Date
}

while ($running) {
    # Messages de HubForge (sans bloquer la boucle).
    if ($pending.IsCompleted) {
        $line = $pending.Result
        if ($null -eq $line) { break }
        try { $msg = $line | ConvertFrom-Json } catch { $msg = $null }
        if ($msg) {
            switch ($msg.type) {
                'init'     { Apply $msg.settings }
                'settings' { Apply $msg.settings }
                'action'   { if ($msg.event -eq 'ping_now') { $next = Get-Date; $key = $msg.key } }
                'shutdown' { $running = $false }
            }
        }
        $pending = $stdin.ReadLineAsync()
    }
    if ($running -and (Get-Date) -ge $next) {
        $ok = $false
        try {
            $r = $ping.Send($target, 1500)
            if ($r.Status -eq 'Success') {
                Send @{ type = 'set_var'; name = 'ms'; value = "$($r.RoundtripTime) ms" }
                $ok = $true
            }
        } catch { }
        if (-not $ok) { Send @{ type = 'set_var'; name = 'ms'; value = '—' } }
        Send @{ type = 'set_var'; name = 'down'; value = $(if ($ok) { '0' } else { '1' }) }
        if ($key) {
            Send @{ type = $(if ($ok) { 'ok' } else { 'alert' }); key = $key }
            $key = $null
        }
        $next = (Get-Date).AddSeconds($every)
    }
    Start-Sleep -Milliseconds 100
}
