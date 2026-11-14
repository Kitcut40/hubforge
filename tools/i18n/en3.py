EN3 = {
"Mise à jour": "Update", "Regarde ton MasterHUB : le jeu tourne sur les touches.": "Look at your MasterHUB: the game runs on the keys.", "Quitter Doom": "Quit Doom",
"Sur le MasterHUB": "On the MasterHUB", "appui long : quitter": "long press: quit", "Avancer": "Forward", "Entrée / Oui": "Enter / Yes", "Tourner ←": "Turn ←", "TIRER": "FIRE",
"Arme suivante": "Next weapon", "Tourner →": "Turn →", "Pas de côté ←": "Strafe ←", "Reculer": "Back", "Pas de côté →": "Strafe →",
"Molette gauche : viser · molette droite : avancer / reculer · LED des faders : ta vie.": "Left wheel: aim · right wheel: forward / back · fader LEDs: your health.",
"Au clavier": "On the keyboard",
"Flèches ou ZQSD : bouger · Ctrl ou F : tirer · Espace : ouvrir · Maj : courir · 1 à 7 ou E : armes · Tab : carte · Échap : menu": "Arrows or WASD: move · Ctrl or F: fire · Space: open · Shift: run · 1 to 7 or E: weapons · Tab: map · Esc: menu",
"HubForge reconnaît la position et le sens des modules d'après leur branchement dans la base. Si un branchement n'est pas encore connu, tu peux le saisir ici ; envoie-nous le rapport de diagnostic pour qu'il soit reconnu pour tout le monde.": "HubForge recognizes the position and direction of the modules from how they're plugged into the base. If a connection isn't known yet, you can enter it here; send us the diagnostics report so it gets recognized for everyone.",
"Ne rien créer ; seul le style choisi est appliqué.": "Create nothing; only the chosen style is applied.",
"Ton profil reste tel quel ; il prend seulement le style choisi.": "Your profile stays as it is; it only takes the chosen style.",
"Un nouveau profil est créé : ton profil actuel n'est pas modifié. Les faders contrôlent le volume général, le micro et des applications courantes ; tout se change ensuite.": "A new profile is created: your current profile isn't changed. The faders control the master volume, the mic and common applications; everything can be changed later.",
"Langue": "Language", "Langue / Language": "Language / Langue",
"Branche le MasterHUB, puis reviens ici.": "Plug in the MasterHUB, then come back here.",
"Cette touche affiche": "This key shows",
}
EN3.update({
"(aucun)": "(none)", "ex. notepad.exe ou C:\\…\\appli.exe": "e.g. notepad.exe or C:\\…\\app.exe",
"Action lancée quand la touche est dans l'état 1 ; elle passe ensuite à l'état 2.": "Action run when the key is in state 1; it then switches to state 2.",
"Action lancée quand la touche est dans l'état 2 ; elle revient ensuite à l'état 1.": "Action run when the key is in state 2; it then goes back to state 1.",
"Rien n'est envoyé : « Copier le rapport » met ces informations dans le presse-papiers pour que tu puisses les joindre à une demande d'aide.": "Nothing is sent: \"Copy the report\" puts this information on the clipboard so you can attach it to a help request.",
"Journal": "Log", "Cette touche est libre.": "This key is free.",
})

# 0.23 : addons nouvelle génération
EN3.update({
"Addons": "Addons", "Actions, pages et programmes": "Actions, pages and programs",
"Cet addon contient un programme": "This addon includes a program",
"Autoriser ce programme": "Allow this program",
"Programme autorisé : il démarre dans quelques secondes.": "Program allowed: it starts in a few seconds.",
"Programme en marche": "Program running", "Programme en cours de démarrage…": "Program starting…",
"Programme arrêté (addon désactivé)": "Program stopped (addon disabled)", "En attente de ton accord": "Waiting for your approval",
"Programme en erreur": "Program error", "Relancer": "Restart", "Retirer l'autorisation": "Revoke permission",
"Le programme ne sera plus lancé": "The program won't be started anymore",
"Action d'un addon": "Addon action", "Addon": "Addon", "Choisir un addon…": "Choose an addon…",
"Aucun addon installé n'a de programme.": "No installed addon has a program.",
"Événement": "Event", "ex. ping_now": "e.g. ping_now",
"Le nom que le programme de l'addon attend (voir sa documentation).": "The name the addon's program expects (see its documentation).",
"Son programme ne tourne pas : ouvre la fiche de l'addon (bouton Addons en haut).": "Its program isn't running: open the addon's card (Addons button at the top).",
"État piloté par une variable": "State driven by a variable",
"Action lancée quand la touche est dans l'état 1 (l'état suit la variable).": "Action run when the key is in state 1 (the state follows the variable).",
"Action lancée quand la touche est dans l'état 2 (l'état suit la variable).": "Action run when the key is in state 2 (the state follows the variable).",
"La touche passe à l'état 2 quand la variable vaut autre chose que vide, 0, false ou off (par exemple le direct d'OBS, envoyé par son addon). L'appui ne change alors plus l'état lui-même.": "The key switches to state 2 when the variable is anything other than empty, 0, false or off (for example OBS going live, sent by its addon). Pressing the key then no longer changes the state itself.",
})
