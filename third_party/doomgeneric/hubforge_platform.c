// HubForge — couche « plateforme » de doomgeneric.
//
// Ce fichier relie le moteur de Doom à HubForge : l'image, l'heure, le
// clavier et l'état du joueur passent par des fonctions écrites en Rust
// (apps/hubforge/src/doom.rs). Distribué sous la GPL, version 2 ou
// (à votre choix) toute version ultérieure, comme le reste de doomgeneric.

#include "doomgeneric.h"
#include "doomstat.h"
#include "d_player.h"
#include "m_controls.h"

// Fournies par HubForge (Rust).
extern void hf_draw_frame(const uint32_t *pixels, int width, int height);
extern void hf_sleep_ms(uint32_t ms);
extern uint32_t hf_ticks_ms(void);
extern int hf_get_key(int *pressed, unsigned char *key);

void DG_Init(void) {}

void DG_DrawFrame(void) { hf_draw_frame(DG_ScreenBuffer, DOOMGENERIC_RESX, DOOMGENERIC_RESY); }

void DG_SleepMs(uint32_t ms) { hf_sleep_ms(ms); }

uint32_t DG_GetTicksMs(void) { return hf_ticks_ms(); }

int DG_GetKey(int *pressed, unsigned char *key) { return hf_get_key(pressed, key); }

void DG_SetWindowTitle(const char *title) { (void)title; }

// État du joueur, pour les LED (vie, dégâts reçus, bonus ramassés).
int hf_player_health(void) { return gamestate == GS_LEVEL ? players[consoleplayer].health : -1; }
int hf_player_damage(void) { return players[consoleplayer].damagecount; }
int hf_player_bonus(void) { return players[consoleplayer].bonuscount; }

// Lance Doom et fait tourner sa boucle (ne revient jamais : quitter le jeu
// termine le processus).
void hf_doom_run(int argc, char **argv)
{
    doomgeneric_Create(argc, argv);
    // Touches pour changer d'arme (absentes du Doom d'origine).
    key_nextweapon = ']';
    key_prevweapon = '[';
    for (;;)
        doomgeneric_Tick();
}
