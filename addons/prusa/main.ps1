# Programme de l'addon « Imprimante Prusa » pour HubForge.
#
# Il interroge l'imprimante par PrusaLink (API locale documentée par Prusa :
# https://github.com/prusa3d/Prusa-Link-Web/blob/master/spec/openapi.yaml),
# publie des variables {prusa.…} et DESSINE les touches de l'addon en SVG,
# dans le style de l'écran de l'imprimante (police DejaVu Sans Mono, icônes
# de l'interface Prusa, orange Prusa).
#
# HubForge écrit une ligne JSON par message sur l'entrée standard ; on lui
# répond par des lignes JSON sur la sortie standard. Compatible Windows
# PowerShell 5.1 et PowerShell 7.

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
Add-Type -AssemblyName System.Net.Http

# ----------------------------------------------------------------------
# Dialogue avec HubForge
# ----------------------------------------------------------------------

function Send($obj) {
    [Console]::Out.WriteLine(($obj | ConvertTo-Json -Compress -Depth 5))
    [Console]::Out.Flush()
}

$script:vars = @{}
function SetVar($name, $value) {
    $v = [string]$value
    if ($script:vars[$name] -ne $v) {
        $script:vars[$name] = $v
        Send @{ type = 'set_var'; name = $name; value = $v }
    }
}

$script:drawn = @{}
function Draw($canvas, $svg) {
    # N'envoie que les dessins qui ont changé.
    if ($script:drawn[$canvas] -ne $svg) {
        $script:drawn[$canvas] = $svg
        Send @{ type = 'draw'; canvas = $canvas; svg = $svg }
    }
}

function Notify($text, $level) {
    if ($script:notifyOn) { Send @{ type = 'notify'; text = $text; level = $level } }
}

function Mark($key, $ok) {
    if ($key) { Send @{ type = $(if ($ok) { 'ok' } else { 'alert' }); key = $key } }
}

function Log($text) { Send @{ type = 'log'; text = $text } }

$script:en = $false
function T($fr, $en) { if ($script:en) { $en } else { $fr } }

# ----------------------------------------------------------------------
# Réglages
# ----------------------------------------------------------------------

$script:host_ = ''
$script:user = 'maker'
$script:password = ''
$script:every = 3
$script:customNozzle = 240
$script:customBed = 90
$script:notifyOn = $true
$script:client = $null
$script:next = Get-Date
$script:gap = 14

# Températures du menu de préchauffage du firmware Prusa (buse, plateau).
$presets = [ordered]@{
    'PLA'  = @(215, 60)
    'PETG' = @(230, 85)
    'ASA'  = @(260, 100)
    'PC'   = @(275, 100)
    'PVB'  = @(215, 75)
    'ABS'  = @(255, 100)
    'HIPS' = @(220, 100)
    'PP'   = @(240, 100)
    'FLEX' = @(240, 50)
    'PA'   = @(285, 100)
}

function NewClient {
    if ($script:client) { $script:client.Dispose() }
    $script:client = $null
    if (-not $script:host_) { return }
    $base = "http://$($script:host_)/"
    $handler = New-Object System.Net.Http.HttpClientHandler
    # PrusaLink : authentification « Digest » (utilisateur maker + mot de passe).
    $cache = New-Object System.Net.CredentialCache
    $cache.Add([Uri]$base, 'Digest', (New-Object System.Net.NetworkCredential($script:user, $script:password)))
    $handler.Credentials = $cache
    $handler.PreAuthenticate = $true
    $c = New-Object System.Net.Http.HttpClient($handler)
    $c.BaseAddress = [Uri]$base
    $c.Timeout = [TimeSpan]::FromSeconds(4)
    # Les firmwares récents acceptent aussi le mot de passe en clé d'API.
    if ($script:password) { [void]$c.DefaultRequestHeaders.TryAddWithoutValidation('X-Api-Key', $script:password) }
    $script:client = $c
}

function Apply($s) {
    if ($null -eq $s) { return }
    $script:host_ = ([string]$s.host).Trim() -replace '^https?://', '' -replace '/+$', ''
    if ($s.user) { $script:user = [string]$s.user } else { $script:user = 'maker' }
    $script:password = [string]$s.password
    if ($s.every) { $script:every = [Math]::Max(1, [int]$s.every) }
    if ($s.custom_nozzle) { $script:customNozzle = [Math]::Min(300, [Math]::Max(0, [int]$s.custom_nozzle)) }
    if ($s.custom_bed) { $script:customBed = [Math]::Min(120, [Math]::Max(0, [int]$s.custom_bed)) }
    $script:notifyOn = -not ($s.notify -eq $false)
    NewClient
    $script:next = Get-Date
}

function ApplyLayout($l) {
    if ($l -and $null -ne $l.gap) { $script:gap = [int]$l.gap }
}

# ----------------------------------------------------------------------
# Appels à PrusaLink
# ----------------------------------------------------------------------

# Retourne @{ code = 200; json = …; bytes = …; error = '…' }
function Api($method, $path, $body, $headers, [switch]$raw) {
    if (-not $script:client) { return @{ code = 0; error = (T "adresse IP non renseignée" "printer IP not set") } }
    try {
        $req = [System.Net.Http.HttpRequestMessage]::new([System.Net.Http.HttpMethod]::new($method), $path)
        if ($null -ne $body) {
            $req.Content = [System.Net.Http.ByteArrayContent]::new([byte[]]$body)
            $req.Content.Headers.ContentType = New-Object System.Net.Http.Headers.MediaTypeHeaderValue('application/octet-stream')
        }
        if ($headers) { foreach ($k in $headers.Keys) { [void]$req.Headers.TryAddWithoutValidation($k, $headers[$k]) } }
        $res = $script:client.SendAsync($req).GetAwaiter().GetResult()
        $code = [int]$res.StatusCode
        $err = $null
        if ($code -eq 401) { $err = T "mot de passe PrusaLink refusé" "PrusaLink password rejected" }
        elseif ($code -ge 400) { $err = "HTTP $code" }
        if ($raw) {
            $bytes = $res.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
            return @{ code = $code; bytes = $bytes; type = [string]$res.Content.Headers.ContentType; error = $err }
        }
        $text = $res.Content.ReadAsStringAsync().GetAwaiter().GetResult()
        $json = $null
        if ($text -and $text.TrimStart().StartsWith('{')) { try { $json = $text | ConvertFrom-Json } catch { } }
        return @{ code = $code; json = $json; error = $err }
    } catch {
        return @{ code = 0; error = (T "imprimante injoignable" "printer unreachable") }
    }
}

# ----------------------------------------------------------------------
# Mise en forme
# ----------------------------------------------------------------------

function Duration($sec) {
    if ($null -eq $sec -or $sec -lt 0) { return '—' }
    $sec = [int]$sec
    $h = [Math]::Floor($sec / 3600)
    $m = [Math]::Floor(($sec % 3600) / 60)
    if ($h -gt 0) { return ('{0}h{1:00}' -f $h, $m) }
    if ($m -gt 0) { return "${m}m" }
    return "<1m"
}

function Eta($sec) {
    if ($null -eq $sec -or $sec -lt 0) { return '—' }
    return (Get-Date).AddSeconds([int]$sec).ToString('HH:mm')
}

function EtaDay($sec) {
    if ($null -eq $sec -or $sec -lt 0) { return '' }
    $end = (Get-Date).AddSeconds([int]$sec)
    if ($end.Date -eq (Get-Date).Date) { return '' }
    if ($end.Date -eq (Get-Date).Date.AddDays(1)) { return (T "demain" "tomorrow") }
    return $end.ToString('MM-dd')
}

function StateText($state) {
    switch ($state) {
        'IDLE'      { return (T "PRÊTE" "READY") }
        'READY'     { return (T "PRÊTE" "READY") }
        'BUSY'      { return (T "OCCUPÉE" "BUSY") }
        'PRINTING'  { return (T "IMPRIME" "PRINTING") }
        'PAUSED'    { return (T "PAUSE" "PAUSED") }
        'FINISHED'  { return (T "TERMINÉ" "FINISHED") }
        'STOPPED'   { return (T "ARRÊTÉ" "STOPPED") }
        'ERROR'     { return (T "ERREUR" "ERROR") }
        'ATTENTION' { return "ATTENTION" }
        'OFFLINE'   { return (T "HORS LIGNE" "OFFLINE") }
        default     { return [string]$state }
    }
}

function Short($s, $max) {
    $s = [string]$s
    $s = $s -replace '\.(b?gcode|gco)$', ''
    if ($s.Length -le $max) { return $s }
    return $s.Substring(0, $max - 1) + '…'
}

# ----------------------------------------------------------------------
# Dessin (SVG) — style de l'écran Prusa
# ----------------------------------------------------------------------

$ORANGE = '#FA6831'
$TILE = '#5B5B5B'
$TRACK = '#3A3A3A'
$FONT = 'font-family="DejaVu Sans Mono" font-weight="bold"'

function Esc($s) { return ([string]$s).Replace('&', '&amp;').Replace('<', '&lt;').Replace('>', '&gt;').Replace('"', '&quot;') }

# Largeur d'une zone de n touches (bords du module compris).
function ZoneW($n) { return $n * 72 + ($n - 1) * $script:gap }

function Svg($w, $h, $body, $bg) {
    if (-not $bg) { $bg = '#000000' }
    return "<svg xmlns=""http://www.w3.org/2000/svg"" xmlns:xlink=""http://www.w3.org/1999/xlink"" width=""$w"" height=""$h"" viewBox=""0 0 $w $h""><rect width=""$w"" height=""$h"" fill=""$bg""/>$body</svg>"
}

function Txt($x, $y, $s, $size, $color, $anchor) {
    if (-not $anchor) { $anchor = 'middle' }
    return "<text x=""$x"" y=""$y"" $FONT font-size=""$size"" fill=""$color"" text-anchor=""$anchor"">$(Esc $s)</text>"
}

# Taille de texte qui tient dans une largeur (police à chasse fixe : 0,6 em par caractère).
function Fit($s, $size, $width) {
    $n = ([string]$s).Length
    if ($n -eq 0) { return $size }
    return [Math]::Min($size, [Math]::Floor($width / (0.61 * $n)))
}

function Img($href, $x, $y, $w, $h, $pixel) {
    $r = if ($pixel) { ' image-rendering="optimizeSpeed"' } else { '' }
    return "<image xlink:href=""$href"" href=""$href"" x=""$x"" y=""$y"" width=""$w"" height=""$h""$r/>"
}

# Tuile du menu de l'imprimante : icône Prusa 80×80 + nom en dessous.
function Tile($icon, $label, $focused, $dim) {
    $bg = $TILE; $fg = '#FFFFFF'
    if ($focused) { $bg = '#FFFFFF'; $fg = '#111111' }
    $op = if ($dim) { ' opacity="0.35"' } else { '' }
    $icn = $icon
    $foc = $icon -replace '\.png$', '_focused.png'
    if ($focused -and (Test-Path (Join-Path $PSScriptRoot "icons/prusa/$foc"))) { $icn = $foc }
    $size = Fit $label 12 66
    $body = "<g$op>" + (Img "icons/prusa/$icn" 9 1 54 54) + (Txt 36 67 $label $size $fg) + "</g>"
    return Svg 72 72 $body $bg
}

# Valeur en grand avec petit intitulé orange au-dessus.
function ValueKey($caption, $value, $sub) {
    $size = Fit $value 24 68
    $body = (Txt 36 19 $caption 12 $ORANGE) + (Txt 36 47 $value $size '#FFFFFF')
    if ($sub) { $body += Txt 36 65 $sub 11 '#FFFFFF' }
    return Svg 72 72 $body
}

# Température : petite icône Prusa, valeur, barre de chauffe en bas.
function TempKey($icon, $cur, $target) {
    if ($null -eq $cur) { $val = '—' } else { $val = "$([Math]::Round([double]$cur))°" }
    $body = (Img "icons/prusa/$icon" 24 4 24 24 $true) + (Txt 36 52 $val (Fit $val 22 68) '#FFFFFF')
    $body += "<rect x=""6"" y=""61"" width=""60"" height=""6"" fill=""$TRACK""/>"
    if ($null -ne $target -and [double]$target -gt 0 -and $null -ne $cur) {
        $p = [Math]::Max(0, [Math]::Min(1, [double]$cur / [double]$target))
        $body += "<rect x=""6"" y=""61"" width=""$([Math]::Round(60 * $p, 1))"" height=""6"" fill=""$ORANGE""/>"
    }
    return Svg 72 72 $body
}

# Préchauffage : nom du filament en grand, températures en orange.
function HeatKey($name, $t, $active) {
    $bg = if ($active) { $TILE } else { '#000000' }
    $body = (Img "icons/prusa/preheat_white_16x16.png" 26 3 20 20 $true)
    $body += Txt 36 45 $name (Fit $name 21 66) '#FFFFFF'
    $temps = "$($t[0])/$($t[1])"
    $body += Txt 36 64 $temps (Fit $temps 13 68) $ORANGE
    return Svg 72 72 $body $bg
}

# Barre d'avancement sur une rangée de 4 touches.
function BarZone($p, $finished) {
    $w = ZoneW 4
    $body = "<rect x=""0"" y=""27"" width=""$w"" height=""18"" fill=""$TRACK""/>"
    if ($finished) { $p = 1 }
    if ($p -gt 0) { $body += "<rect x=""0"" y=""27"" width=""$([Math]::Round($w * $p, 1))"" height=""18"" fill=""$ORANGE""/>" }
    return Svg $w 72 $body
}

# Aperçu de la pièce sur 2 × 2 touches.
function ThumbZone($dataUri, $state) {
    $s = ZoneW 2
    if ($dataUri) {
        $body = "<image xlink:href=""$dataUri"" href=""$dataUri"" x=""0"" y=""0"" width=""$s"" height=""$s"" preserveAspectRatio=""xMidYMid meet""/>"
        return Svg $s $s $body
    }
    # Pas de vignette : icône dans la case en haut à gauche, état en bas à
    # gauche (aucun texte à cheval sur les bords du module).
    $icon = switch ($state) { 'OFFLINE' { 'disconnect_80x80.png' } 'ERROR' { 'error_48x48.png' } 'ATTENTION' { 'warning_48x48.png' } default { 'print_80x80.png' } }
    $word = StateText $state
    $y2 = 72 + $script:gap
    $body = (Img "icons/prusa/$icon" 9 9 54 54) + (Txt 36 ($y2 + 43) $word (Fit $word 15 68) '#FFFFFF')
    return Svg $s $s $body
}

# ----------------------------------------------------------------------
# État et dessins
# ----------------------------------------------------------------------

$script:state = ''
$script:p = $null              # dernier état imprimante (objet printer)
$script:j = $null              # dernier état du travail (objet job)
$script:jobId = $null
$script:jobFile = ''
$script:jobPath = ''           # pour « Ré-imprimer » : "usb/NOM.BGC"
$script:lastFileJob = $null
$script:thumb = $null          # data URI de la vignette
$script:heatWatch = $null
$script:stopArmed = $null
$script:reprintArmed = $null

function DrawStatic {
    Draw 'menu' (Tile 'settings_80x80.png' 'Menu')
    Draw 'heatmenu' (Tile 'preheat_80x80.png' (T 'Préchauf.' 'Preheat'))
    Draw 'back' (Tile 'back_80x80.png' (T 'Retour' 'Back'))
    Draw 'web' (Tile 'info_80x80.png' 'PrusaLink')
    Draw 'refresh' (Tile 'set_ready_80x80.png' (T 'Actualiser' 'Refresh'))
    $cool = T 'Refroidir' 'Cool down'
    Draw 'cooldown' (Svg 72 72 ((Txt 36 38 'OFF' 24 '#FFFFFF') + (Txt 36 64 $cool (Fit $cool 12 66) '#FFFFFF')) $TILE)
}

function DrawAll {
    $st = $script:state
    $p = $script:p
    $j = $script:j
    $printing = ($st -eq 'PRINTING' -or $st -eq 'PAUSED')
    $idle = @('IDLE', 'READY', 'FINISHED', 'STOPPED') -contains $st

    # Avancement
    $pct = $null
    if ($printing -and $j) { $pct = [Math]::Floor([double]$j.progress) }
    if ($st -eq 'FINISHED') { $pct = 100 }
    $pctText = if ($null -ne $pct) { "$pct%" } else { '—' }
    Draw 'pct' (Svg 72 72 (Txt 36 46 $pctText (Fit $pctText 26 68) '#FFFFFF'))
    Draw 'bar' (BarZone $(if ($null -ne $pct) { $pct / 100 } else { 0 }) ($st -eq 'FINISHED'))

    # Temps
    if ($printing -and $j) {
        Draw 'remaining' (ValueKey (T 'RESTE' 'LEFT') (Duration $j.time_remaining))
        Draw 'eta' (ValueKey (T 'FIN' 'END') (Eta $j.time_remaining) (EtaDay $j.time_remaining))
    } else {
        Draw 'remaining' (ValueKey (T 'RESTE' 'LEFT') '—')
        Draw 'eta' (ValueKey (T 'FIN' 'END') '—')
    }

    # Températures
    if ($p) {
        Draw 'nozzle' (TempKey 'nozzle_16x16.png' $p.temp_nozzle $p.target_nozzle)
        Draw 'bed' (TempKey 'heatbed_16x16.png' $p.temp_bed $p.target_bed)
    } else {
        Draw 'nozzle' (TempKey 'nozzle_16x16.png' $null $null)
        Draw 'bed' (TempKey 'heatbed_16x16.png' $null $null)
    }

    # Aperçu et état
    $showThumb = if ($printing -or $st -eq 'FINISHED') { $script:thumb } else { $null }
    Draw 'thumb' (ThumbZone $showThumb $st)
    $stIcon = switch ($st) { 'OFFLINE' { 'disconnect_80x80.png' } 'PAUSED' { 'pause_80x80.png' } default { 'print_80x80.png' } }
    Draw 'status' (Tile $stIcon (StateText $st) $printing)

    # Commandes
    if ($st -eq 'PAUSED') { Draw 'pause' (Tile 'resume_80x80.png' (T 'Reprendre' 'Resume') $true) }
    else { Draw 'pause' (Tile 'pause_80x80.png' 'Pause' $false (-not $printing)) }
    $armed = $script:stopArmed -and ((Get-Date) - $script:stopArmed).TotalSeconds -le 4
    Draw 'stop' (Tile 'stop_80x80.png' $(if ($armed) { T 'Confirmer' 'Confirm' } else { T 'Arrêter' 'Stop' }) $armed (-not $printing))
    $rArmed = $script:reprintArmed -and ((Get-Date) - $script:reprintArmed).TotalSeconds -le 4
    Draw 'reprint' (Tile 'reprint_80x80.png' $(if ($rArmed) { T 'Confirmer' 'Confirm' } else { T 'Relancer' 'Reprint' }) $rArmed (-not ($idle -and $script:jobPath)))

    # Préchauffage : le filament dont les températures sont en cours est mis en avant.
    $tn = 0; $tb = 0
    if ($p) { $tn = [double]$p.target_nozzle; $tb = [double]$p.target_bed }
    foreach ($name in $presets.Keys) {
        $t = $presets[$name]
        Draw "heat_$($name.ToLower())" (HeatKey $name $t ($tn -eq $t[0] -and $tb -eq $t[1]))
    }
    $ct = @($script:customNozzle, $script:customBed)
    Draw 'heat_custom' (HeatKey (T 'PERSO' 'CUSTOM') $ct ($tn -eq $ct[0] -and $tb -eq $ct[1] -and $tn -gt 0))
}

function FetchThumb($file) {
    $ref = $null
    if ($file.refs -and $file.refs.thumbnail) { $ref = [string]$file.refs.thumbnail }
    if (-not $ref) { return $null }
    $r = Api 'GET' ($ref.TrimStart('/')) $null $null -raw
    if ($r.code -ne 200 -or -not $r.bytes -or $r.bytes.Length -lt 50) { return $null }
    $mime = if ($r.type -match 'jpe?g') { 'image/jpeg' } else { 'image/png' }
    return "data:$mime;base64," + [Convert]::ToBase64String($r.bytes)
}

function Poll {
    $r = Api 'GET' 'api/v1/status' $null $null
    if ($r.code -ne 200 -or -not $r.json) {
        SetVar 'status' $(if ($r.code -eq 401) { T "Mot de passe ?" "Password?" } elseif (-not $script:host_) { T "À configurer" "Set me up" } else { T "Hors ligne" "Offline" })
        SetVar 'state' 'OFFLINE'
        SetVar 'offline' '1'
        foreach ($n in 'printing', 'paused') { SetVar $n '0' }
        foreach ($n in 'progress', 'remaining', 'eta', 'nozzle', 'bed', 'file') { SetVar $n '—' }
        if ($script:state -ne 'OFFLINE') { Log "PrusaLink : $($r.error)" }
        $script:state = 'OFFLINE'
        $script:p = $null; $script:j = $null
        DrawAll
        return
    }
    $p = $r.json.printer
    $j = $r.json.job
    $state = [string]$p.state
    $printing = ($state -eq 'PRINTING' -or $state -eq 'PAUSED')

    # Infos du travail (nom, chemin, vignette) : une fois par travail.
    if ($j -and $j.id -and $script:lastFileJob -ne $j.id) {
        $jr = Api 'GET' 'api/v1/job' $null $null
        if ($jr.code -eq 200 -and $jr.json -and $jr.json.file) {
            $f = $jr.json.file
            $name = $f.display_name
            if (-not $name) { $name = $f.name }
            $script:jobFile = [string]$name
            if ($f.path -and $f.name) { $script:jobPath = (([string]$f.path).Trim('/') + '/' + [string]$f.name) }
            $script:thumb = FetchThumb $f
            $script:lastFileJob = $j.id
        }
    }

    SetVar 'offline' '0'
    SetVar 'state' $state
    SetVar 'printing' $(if ($printing) { '1' } else { '0' })
    SetVar 'paused' $(if ($state -eq 'PAUSED') { '1' } else { '0' })
    SetVar 'nozzle' $(if ($null -ne $p.temp_nozzle) { "$([Math]::Round([double]$p.temp_nozzle))°" } else { '—' })
    SetVar 'bed' $(if ($null -ne $p.temp_bed) { "$([Math]::Round([double]$p.temp_bed))°" } else { '—' })
    if ($j -and $printing) {
        $pct = [Math]::Floor([double]$j.progress)
        SetVar 'progress' "$pct%"
        SetVar 'remaining' (Duration $j.time_remaining)
        SetVar 'eta' (Eta $j.time_remaining)
        SetVar 'status' $(if ($state -eq 'PAUSED') { T "En pause" "Paused" } else { T "Impression $pct%" "Printing $pct%" })
        $script:jobId = $j.id
        SetVar 'file' (Short $script:jobFile 18)
    } else {
        SetVar 'status' (StateText $state)
        SetVar 'progress' $(if ($state -eq 'FINISHED') { '100%' } else { '—' })
        SetVar 'remaining' '—'
        SetVar 'eta' '—'
        SetVar 'file' $(if ($script:jobFile) { Short $script:jobFile 18 } else { '—' })
        if ($state -ne 'FINISHED') { $script:jobId = $null }
    }

    # Changements d'état à signaler.
    $before = $script:state
    if ($before -and $before -ne $state -and $before -ne 'OFFLINE') {
        if ($state -eq 'FINISHED') { Notify (T "Impression terminée : $(Short $script:jobFile 40)" "Print finished: $(Short $script:jobFile 40)") 'info' }
        elseif ($state -eq 'ERROR') { Notify (T "L'imprimante signale une erreur" "The printer reports an error") 'error' }
        elseif ($state -eq 'ATTENTION') { Notify (T "L'imprimante demande ton attention" "The printer needs your attention") 'error' }
        elseif ($state -eq 'STOPPED' -and ($before -eq 'PRINTING' -or $before -eq 'PAUSED')) { Notify (T "Impression arrêtée" "Print stopped") 'info' }
        elseif ($state -eq 'PAUSED') { Notify (T "Impression en pause" "Print paused") 'info' }
    }
    $script:state = $state
    $script:p = $p
    $script:j = $j

    # Préchauffage : prévenir quand la température est atteinte.
    if ($script:heatWatch -and $null -ne $p.temp_nozzle) {
        $w = $script:heatWatch
        if ([double]$p.temp_nozzle -ge $w.nozzle - 2 -and [double]$p.temp_bed -ge $w.bed - 2) {
            Notify (T "Préchauffage $($w.name) atteint : $([Math]::Round([double]$p.temp_nozzle))° / $([Math]::Round([double]$p.temp_bed))°" "$($w.name) preheat reached: $([Math]::Round([double]$p.temp_nozzle))° / $([Math]::Round([double]$p.temp_bed))°") 'info'
            $script:heatWatch = $null
        } elseif ($printing -or ((Get-Date) - $w.since).TotalMinutes -gt 20) {
            $script:heatWatch = $null
        }
    }
    DrawAll
}

# ----------------------------------------------------------------------
# Commandes
# ----------------------------------------------------------------------

function IsIdle { return @('IDLE', 'READY', 'FINISHED', 'STOPPED') -contains $script:state }

# Envoie un petit fichier G-code et le lance aussitôt (préchauffage,
# refroidissement) : PrusaLink n'a pas de commande « température ».
# DÉSACTIVÉ dans le manifeste (2.1.0) : l'imprimante s'arrête sur un écran à
# valider avant de lancer le fichier, ce qui rend la touche inutile.
function RunGcode($name, $lines) {
    $st = Api 'GET' 'api/v1/storage' $null $null
    $storage = 'usb'
    if ($st.code -eq 200 -and $st.json -and $st.json.storage_list) {
        $s = $st.json.storage_list | Where-Object { $_.available -and -not $_.read_only } | Select-Object -First 1
        if ($s -and $s.path) { $storage = ([string]$s.path).Trim('/') }
    }
    $text = ($lines -join "`n") + "`n"
    $bytes = [Text.Encoding]::ASCII.GetBytes($text)
    return Api 'PUT' "api/v1/files/$storage/$name" $bytes @{ 'Print-After-Upload' = '?1'; 'Overwrite' = '?1' }
}

function Preheat($filament, $key) {
    if (-not (IsIdle)) {
        Notify (T "Préchauffage impossible : l'imprimante n'est pas à l'arrêt" "Can't preheat: the printer isn't idle") 'error'
        Mark $key $false
        return
    }
    if ($filament -eq 'CUSTOM') { $t = @($script:customNozzle, $script:customBed); $label = T "perso" "custom" }
    else { $t = $presets[$filament]; $label = $filament }
    if (-not $t) { Mark $key $false; return }
    $r = RunGcode 'hubforge-preheat.gcode' @("; HubForge - prechauffage $label", "M104 S$($t[0])", "M140 S$($t[1])")
    if ($r.code -ge 200 -and $r.code -lt 300) {
        Mark $key $true
        $script:heatWatch = @{ nozzle = $t[0]; bed = $t[1]; name = $label; since = Get-Date }
        $script:next = (Get-Date).AddSeconds(1)
    } else {
        Notify (T "Préchauffage refusé : $($r.error)" "Preheat refused: $($r.error)") 'error'
        Mark $key $false
    }
}

function Cooldown($key) {
    if (-not (IsIdle)) {
        Notify (T "Refroidir : l'imprimante n'est pas à l'arrêt" "Cool down: the printer isn't idle") 'error'
        Mark $key $false
        return
    }
    $r = RunGcode 'hubforge-cooldown.gcode' @("; HubForge - refroidir", "M104 S0", "M140 S0")
    $script:heatWatch = $null
    Mark $key ($r.code -ge 200 -and $r.code -lt 300)
    if ($r.error) { Notify (T "Refroidir : $($r.error)" "Cool down: $($r.error)") 'error' }
    $script:next = (Get-Date).AddSeconds(1)
}

function PauseToggle($key) {
    if (-not $script:jobId) { Mark $key $false; return }
    if ($script:state -eq 'PAUSED') { $r = Api 'PUT' "api/v1/job/$($script:jobId)/resume" $null $null }
    elseif ($script:state -eq 'PRINTING') { $r = Api 'PUT' "api/v1/job/$($script:jobId)/pause" $null $null }
    else { Mark $key $false; return }
    Mark $key ($r.code -ge 200 -and $r.code -lt 300)
    if ($r.error) { Notify "Pause : $($r.error)" 'error' }
    $script:next = (Get-Date).AddSeconds(1)
}

function StopPrint($key) {
    if (-not $script:jobId -or -not ($script:state -eq 'PRINTING' -or $script:state -eq 'PAUSED')) { Mark $key $false; return }
    # Double appui obligatoire : un arrêt ne se rattrape pas.
    if (-not $script:stopArmed -or ((Get-Date) - $script:stopArmed).TotalSeconds -gt 4) {
        $script:stopArmed = Get-Date
        Notify (T "Appuie encore sur « Arrêter » pour arrêter l'impression" "Press Stop again to cancel the print") 'info'
        DrawAll
        return
    }
    $script:stopArmed = $null
    $r = Api 'DELETE' "api/v1/job/$($script:jobId)" $null $null
    Mark $key ($r.code -ge 200 -and $r.code -lt 300)
    if ($r.error) { Notify (T "Arrêt refusé : $($r.error)" "Stop refused: $($r.error)") 'error' }
    $script:next = (Get-Date).AddSeconds(1)
}

function Reprint($key) {
    if (-not (IsIdle) -or -not $script:jobPath) { Mark $key $false; return }
    if (-not $script:reprintArmed -or ((Get-Date) - $script:reprintArmed).TotalSeconds -gt 4) {
        $script:reprintArmed = Get-Date
        Notify (T "Appuie encore pour ré-imprimer $(Short $script:jobFile 30)" "Press again to reprint $(Short $script:jobFile 30)") 'info'
        DrawAll
        return
    }
    $script:reprintArmed = $null
    $r = Api 'POST' "api/v1/files/$($script:jobPath)" $null $null
    Mark $key ($r.code -ge 200 -and $r.code -lt 300)
    if ($r.error) { Notify (T "Ré-impression refusée : $($r.error)" "Reprint refused: $($r.error)") 'error' }
    $script:next = (Get-Date).AddSeconds(1)
}

# ----------------------------------------------------------------------
# Boucle principale
# ----------------------------------------------------------------------

$running = $true
$stdin = New-Object System.IO.StreamReader([Console]::OpenStandardInput(), (New-Object System.Text.UTF8Encoding $false))
$pending = $stdin.ReadLineAsync()
$armedRedraw = $false

while ($running) {
    if ($pending.IsCompleted) {
        $line = $pending.Result
        if ($null -eq $line) { break }
        try { $msg = $line | ConvertFrom-Json } catch { $msg = $null }
        if ($msg) {
            switch ($msg.type) {
                'init' {
                    $script:en = ([string]$msg.lang -eq 'en')
                    ApplyLayout $msg.layout
                    Apply $msg.settings
                    DrawStatic
                    DrawAll
                }
                'settings' { Apply $msg.settings; $script:state = ''; DrawAll }
                'layout' { ApplyLayout $msg.layout; $script:drawn = @{}; DrawStatic; DrawAll }
                'action' {
                    $key = $msg.key
                    switch ($msg.event) {
                        'refresh'      { $script:next = Get-Date; Mark $key ($script:state -ne 'OFFLINE') }
                        'preheat'      { Preheat ([string]$msg.data.filament) $key }
                        'cooldown'     { Cooldown $key }
                        'pause_toggle' { PauseToggle $key }
                        'stop'         { StopPrint $key }
                        'reprint'      { Reprint $key }
                        'open_web'     { if ($script:host_) { Start-Process "http://$($script:host_)/" } else { Mark $key $false } }
                    }
                }
                'shutdown' { $running = $false }
            }
        }
        $pending = $stdin.ReadLineAsync()
    }
    if ($running -and (Get-Date) -ge $script:next) {
        Poll
        $script:next = (Get-Date).AddSeconds($script:every)
    }
    # Fin des 4 secondes de confirmation : la tuile reprend son nom.
    $isArmed = ($script:stopArmed -and ((Get-Date) - $script:stopArmed).TotalSeconds -le 4) -or ($script:reprintArmed -and ((Get-Date) - $script:reprintArmed).TotalSeconds -le 4)
    if ($armedRedraw -and -not $isArmed) { DrawAll }
    $armedRedraw = $isArmed
    Start-Sleep -Milliseconds 100
}

if ($script:client) { $script:client.Dispose() }
