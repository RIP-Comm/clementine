//! Pokemon Debugger Tool
//!
//! A debug/cheat window for viewing and editing Pokemon game data in real-time.
//! Displays party Pokemon, money, items, and allows editing values.
use std::sync::{Arc, Mutex};

use crate::emu_thread::{EmuCommand, EmuHandle};
use crate::ui_traits::UiTool;

/// Size of a Pokemon structure in the party (full 100 bytes).
const POKEMON_SIZE_BYTES: usize = 100;
/// Size of a Pokemon structure as u32 for address arithmetic.
const POKEMON_SIZE: u32 = 100;
/// Maximum number of Pokemon in party.
const PARTY_SIZE: usize = 6;
/// Total bytes to fetch for party data.
const PARTY_BYTES: usize = POKEMON_SIZE_BYTES * PARTY_SIZE;

/// Known game versions and their party addresses.
/// Sources: Bulbapedia, pret/pokeemerald decomp
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameVersion {
    EmeraldEU,
    Unknown,
}

impl GameVersion {
    /// Get the party address for this game version.
    const fn party_address(self) -> u32 {
        match self {
            Self::EmeraldEU | Self::Unknown => 0x0202_44EC,
        }
    }

    /// Display name for the UI.
    const fn display_name(self) -> &'static str {
        match self {
            Self::EmeraldEU => "Emerald (EU)",
            Self::Unknown => "Unknown",
        }
    }

    /// Detect game version from the 4-character game code.
    fn from_game_code(code: &str) -> Self {
        match code {
            "BPEP" | "BPED" | "BPEF" | "BPES" | "BPEI" => Self::EmeraldEU,
            _ => Self::Unknown,
        }
    }

    const ALL: &'static [Self] = &[Self::EmeraldEU, Self::Unknown];
}

/// Pokemon species names indexed by National Dex number.
/// Index 0 is empty, 1 = Bulbasaur, etc.
const SPECIES_NAMES: &[&str] = &[
    "???",        // 0 - No Pokemon
    "Bulbasaur",  // 1
    "Ivysaur",    // 2
    "Venusaur",   // 3
    "Charmander", // 4
    "Charmeleon", // 5
    "Charizard",  // 6
    "Squirtle",   // 7
    "Wartortle",  // 8
    "Blastoise",  // 9
    "Caterpie",   // 10
    "Metapod",    // 11
    "Butterfree", // 12
    "Weedle",     // 13
    "Kakuna",     // 14
    "Beedrill",   // 15
    "Pidgey",     // 16
    "Pidgeotto",  // 17
    "Pidgeot",    // 18
    "Rattata",    // 19
    "Raticate",   // 20
    "Spearow",    // 21
    "Fearow",     // 22
    "Ekans",      // 23
    "Arbok",      // 24
    "Pikachu",    // 25
    "Raichu",     // 26
    "Sandshrew",  // 27
    "Sandslash",  // 28
    "Nidoran♀",   // 29
    "Nidorina",   // 30
    "Nidoqueen",  // 31
    "Nidoran♂",   // 32
    "Nidorino",   // 33
    "Nidoking",   // 34
    "Clefairy",   // 35
    "Clefable",   // 36
    "Vulpix",     // 37
    "Ninetales",  // 38
    "Jigglypuff", // 39
    "Wigglytuff", // 40
    "Zubat",      // 41
    "Golbat",     // 42
    "Oddish",     // 43
    "Gloom",      // 44
    "Vileplume",  // 45
    "Paras",      // 46
    "Parasect",   // 47
    "Venonat",    // 48
    "Venomoth",   // 49
    "Diglett",    // 50
    "Dugtrio",    // 51
    "Meowth",     // 52
    "Persian",    // 53
    "Psyduck",    // 54
    "Golduck",    // 55
    "Mankey",     // 56
    "Primeape",   // 57
    "Growlithe",  // 58
    "Arcanine",   // 59
    "Poliwag",    // 60
    "Poliwhirl",  // 61
    "Poliwrath",  // 62
    "Abra",       // 63
    "Kadabra",    // 64
    "Alakazam",   // 65
    "Machop",     // 66
    "Machoke",    // 67
    "Machamp",    // 68
    "Bellsprout", // 69
    "Weepinbell", // 70
    "Victreebel", // 71
    "Tentacool",  // 72
    "Tentacruel", // 73
    "Geodude",    // 74
    "Graveler",   // 75
    "Golem",      // 76
    "Ponyta",     // 77
    "Rapidash",   // 78
    "Slowpoke",   // 79
    "Slowbro",    // 80
    "Magnemite",  // 81
    "Magneton",   // 82
    "Farfetch'd", // 83
    "Doduo",      // 84
    "Dodrio",     // 85
    "Seel",       // 86
    "Dewgong",    // 87
    "Grimer",     // 88
    "Muk",        // 89
    "Shellder",   // 90
    "Cloyster",   // 91
    "Gastly",     // 92
    "Haunter",    // 93
    "Gengar",     // 94
    "Onix",       // 95
    "Drowzee",    // 96
    "Hypno",      // 97
    "Krabby",     // 98
    "Kingler",    // 99
    "Voltorb",    // 100
    "Electrode",  // 101
    "Exeggcute",  // 102
    "Exeggutor",  // 103
    "Cubone",     // 104
    "Marowak",    // 105
    "Hitmonlee",  // 106
    "Hitmonchan", // 107
    "Lickitung",  // 108
    "Koffing",    // 109
    "Weezing",    // 110
    "Rhyhorn",    // 111
    "Rhydon",     // 112
    "Chansey",    // 113
    "Tangela",    // 114
    "Kangaskhan", // 115
    "Horsea",     // 116
    "Seadra",     // 117
    "Goldeen",    // 118
    "Seaking",    // 119
    "Staryu",     // 120
    "Starmie",    // 121
    "Mr. Mime",   // 122
    "Scyther",    // 123
    "Jynx",       // 124
    "Electabuzz", // 125
    "Magmar",     // 126
    "Pinsir",     // 127
    "Tauros",     // 128
    "Magikarp",   // 129
    "Gyarados",   // 130
    "Lapras",     // 131
    "Ditto",      // 132
    "Eevee",      // 133
    "Vaporeon",   // 134
    "Jolteon",    // 135
    "Flareon",    // 136
    "Porygon",    // 137
    "Omanyte",    // 138
    "Omastar",    // 139
    "Kabuto",     // 140
    "Kabutops",   // 141
    "Aerodactyl", // 142
    "Snorlax",    // 143
    "Articuno",   // 144
    "Zapdos",     // 145
    "Moltres",    // 146
    "Dratini",    // 147
    "Dragonair",  // 148
    "Dragonite",  // 149
    "Mewtwo",     // 150
    "Mew",        // 151
    // Gen 2
    "Chikorita",  // 152
    "Bayleef",    // 153
    "Meganium",   // 154
    "Cyndaquil",  // 155
    "Quilava",    // 156
    "Typhlosion", // 157
    "Totodile",   // 158
    "Croconaw",   // 159
    "Feraligatr", // 160
    "Sentret",    // 161
    "Furret",     // 162
    "Hoothoot",   // 163
    "Noctowl",    // 164
    "Ledyba",     // 165
    "Ledian",     // 166
    "Spinarak",   // 167
    "Ariados",    // 168
    "Crobat",     // 169
    "Chinchou",   // 170
    "Lanturn",    // 171
    "Pichu",      // 172
    "Cleffa",     // 173
    "Igglybuff",  // 174
    "Togepi",     // 175
    "Togetic",    // 176
    "Natu",       // 177
    "Xatu",       // 178
    "Mareep",     // 179
    "Flaaffy",    // 180
    "Ampharos",   // 181
    "Bellossom",  // 182
    "Marill",     // 183
    "Azumarill",  // 184
    "Sudowoodo",  // 185
    "Politoed",   // 186
    "Hoppip",     // 187
    "Skiploom",   // 188
    "Jumpluff",   // 189
    "Aipom",      // 190
    "Sunkern",    // 191
    "Sunflora",   // 192
    "Yanma",      // 193
    "Wooper",     // 194
    "Quagsire",   // 195
    "Espeon",     // 196
    "Umbreon",    // 197
    "Murkrow",    // 198
    "Slowking",   // 199
    "Misdreavus", // 200
    "Unown",      // 201
    "Wobbuffet",  // 202
    "Girafarig",  // 203
    "Pineco",     // 204
    "Forretress", // 205
    "Dunsparce",  // 206
    "Gligar",     // 207
    "Steelix",    // 208
    "Snubbull",   // 209
    "Granbull",   // 210
    "Qwilfish",   // 211
    "Scizor",     // 212
    "Shuckle",    // 213
    "Heracross",  // 214
    "Sneasel",    // 215
    "Teddiursa",  // 216
    "Ursaring",   // 217
    "Slugma",     // 218
    "Magcargo",   // 219
    "Swinub",     // 220
    "Piloswine",  // 221
    "Corsola",    // 222
    "Remoraid",   // 223
    "Octillery",  // 224
    "Delibird",   // 225
    "Mantine",    // 226
    "Skarmory",   // 227
    "Houndour",   // 228
    "Houndoom",   // 229
    "Kingdra",    // 230
    "Phanpy",     // 231
    "Donphan",    // 232
    "Porygon2",   // 233
    "Stantler",   // 234
    "Smeargle",   // 235
    "Tyrogue",    // 236
    "Hitmontop",  // 237
    "Smoochum",   // 238
    "Elekid",     // 239
    "Magby",      // 240
    "Miltank",    // 241
    "Blissey",    // 242
    "Raikou",     // 243
    "Entei",      // 244
    "Suicune",    // 245
    "Larvitar",   // 246
    "Pupitar",    // 247
    "Tyranitar",  // 248
    "Lugia",      // 249
    "Ho-Oh",      // 250
    "Celebi",     // 251
    // Gen 3
    "Treecko",    // 252
    "Grovyle",    // 253
    "Sceptile",   // 254
    "Torchic",    // 255
    "Combusken",  // 256
    "Blaziken",   // 257
    "Mudkip",     // 258
    "Marshtomp",  // 259
    "Swampert",   // 260
    "Poochyena",  // 261
    "Mightyena",  // 262
    "Zigzagoon",  // 263
    "Linoone",    // 264
    "Wurmple",    // 265
    "Silcoon",    // 266
    "Beautifly",  // 267
    "Cascoon",    // 268
    "Dustox",     // 269
    "Lotad",      // 270
    "Lombre",     // 271
    "Ludicolo",   // 272
    "Seedot",     // 273
    "Nuzleaf",    // 274
    "Shiftry",    // 275
    "Taillow",    // 276
    "Swellow",    // 277
    "Wingull",    // 278
    "Pelipper",   // 279
    "Ralts",      // 280
    "Kirlia",     // 281
    "Gardevoir",  // 282
    "Surskit",    // 283
    "Masquerain", // 284
    "Shroomish",  // 285
    "Breloom",    // 286
    "Slakoth",    // 287
    "Vigoroth",   // 288
    "Slaking",    // 289
    "Nincada",    // 290
    "Ninjask",    // 291
    "Shedinja",   // 292
    "Whismur",    // 293
    "Loudred",    // 294
    "Exploud",    // 295
    "Makuhita",   // 296
    "Hariyama",   // 297
    "Azurill",    // 298
    "Nosepass",   // 299
    "Skitty",     // 300
    "Delcatty",   // 301
    "Sableye",    // 302
    "Mawile",     // 303
    "Aron",       // 304
    "Lairon",     // 305
    "Aggron",     // 306
    "Meditite",   // 307
    "Medicham",   // 308
    "Electrike",  // 309
    "Manectric",  // 310
    "Plusle",     // 311
    "Minun",      // 312
    "Volbeat",    // 313
    "Illumise",   // 314
    "Roselia",    // 315
    "Gulpin",     // 316
    "Swalot",     // 317
    "Carvanha",   // 318
    "Sharpedo",   // 319
    "Wailmer",    // 320
    "Wailord",    // 321
    "Numel",      // 322
    "Camerupt",   // 323
    "Torkoal",    // 324
    "Spoink",     // 325
    "Grumpig",    // 326
    "Spinda",     // 327
    "Trapinch",   // 328
    "Vibrava",    // 329
    "Flygon",     // 330
    "Cacnea",     // 331
    "Cacturne",   // 332
    "Swablu",     // 333
    "Altaria",    // 334
    "Zangoose",   // 335
    "Seviper",    // 336
    "Lunatone",   // 337
    "Solrock",    // 338
    "Barboach",   // 339
    "Whiscash",   // 340
    "Corphish",   // 341
    "Crawdaunt",  // 342
    "Baltoy",     // 343
    "Claydol",    // 344
    "Lileep",     // 345
    "Cradily",    // 346
    "Anorith",    // 347
    "Armaldo",    // 348
    "Feebas",     // 349
    "Milotic",    // 350
    "Castform",   // 351
    "Kecleon",    // 352
    "Shuppet",    // 353
    "Banette",    // 354
    "Duskull",    // 355
    "Dusclops",   // 356
    "Tropius",    // 357
    "Chimecho",   // 358
    "Absol",      // 359
    "Wynaut",     // 360
    "Snorunt",    // 361
    "Glalie",     // 362
    "Spheal",     // 363
    "Sealeo",     // 364
    "Walrein",    // 365
    "Clamperl",   // 366
    "Huntail",    // 367
    "Gorebyss",   // 368
    "Relicanth",  // 369
    "Luvdisc",    // 370
    "Bagon",      // 371
    "Shelgon",    // 372
    "Salamence",  // 373
    "Beldum",     // 374
    "Metang",     // 375
    "Metagross",  // 376
    "Regirock",   // 377
    "Regice",     // 378
    "Registeel",  // 379
    "Latias",     // 380
    "Latios",     // 381
    "Kyogre",     // 382
    "Groudon",    // 383
    "Rayquaza",   // 384
    "Jirachi",    // 385
    "Deoxys",     // 386
];

/// Gen 3 internal species index to National Dex number mapping.
/// Gen 3 games use a different internal species ordering than the National Dex.
/// Pokemon 1-251 are the same, but 252-276 are empty, and 277+ are Hoenn Pokemon.
/// This maps internal index -> National Dex number.
/// Reference: <https://bulbapedia.bulbagarden.net/wiki/List_of_Pokémon_by_index_number_in_Generation_III>
#[rustfmt::skip]
const SPECIES_TO_NATIONAL: &[u16] = &[
    // 0 = None
    0,
    // 1-251: Kanto/Johto Pokemon (same as National Dex)
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
    41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60,
    61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80,
    81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100,
    101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116,
    117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132,
    133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148,
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164,
    165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180,
    181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196,
    197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212,
    213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228,
    229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244,
    245, 246, 247, 248, 249, 250, 251,
    // 252-276: Empty/placeholder slots (map to 0)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    // 277-411: Hoenn Pokemon (index -> National Dex)
    // 277=Treecko(252), 278=Grovyle(253), 279=Sceptile(254), 280=Torchic(255), ...
    // 283=Mudkip(258), ...
    252, 253, 254, 255, 256, 257, 258, 259, 260, 261, 262, 263, 264, 265, 266, 267, // 277-292
    268, 269, 270, 271, 272, 273, 274, 275, 290, 291, 292, 276, 277, 285, 286, 327, // 293-308
    278, 279, 283, 284, 320, 321, 300, 301, 352, 343, 344, 299, 324, 302, 339, 340, // 309-324
    370, 341, 342, 349, 350, 318, 319, 328, 329, 330, 296, 297, 309, 310, 322, 323, // 325-340
    363, 364, 365, 331, 332, 361, 362, 337, 338, 298, 325, 326, 311, 312, 303, 307, // 341-356
    308, 333, 334, 360, 355, 356, 315, 287, 288, 289, 316, 317, 357, 293, 294, 295, // 357-372
    366, 367, 368, 359, 353, 354, 336, 335, 369, 304, 305, 306, 351, 313, 314, 345, // 373-388
    346, 347, 348, 280, 281, 282, 371, 372, 373, 374, 375, 376, 377, 378, 379, 382, // 389-404
    383, 384, 380, 381, 385, 386, 358, // 405-411
];

/// Experience required for each level in the "Medium Fast" growth rate (most common).
/// Index 0 = level 1 (0 exp), index 99 = level 100.
/// Formula: n^3 where n is the level.
const MEDIUM_FAST_EXP: [u32; 100] = {
    let mut table = [0u32; 100];
    let mut i: u32 = 0;
    while i < 100 {
        let level = i + 1;
        table[i as usize] = level * level * level;
        i += 1;
    }
    table
};

/// Order table for substructure positions based on personality % 24.
/// Each entry gives [G, A, E, M] positions where G=Growth, A=Attacks, E=EVs, M=Misc.
#[rustfmt::skip]
const SUBSTRUCTURE_ORDERS: [[usize; 4]; 24] = [
    [0, 1, 2, 3], [0, 1, 3, 2], [0, 2, 1, 3], [0, 3, 1, 2], // 0-3
    [0, 2, 3, 1], [0, 3, 2, 1], [1, 0, 2, 3], [1, 0, 3, 2], // 4-7
    [2, 0, 1, 3], [3, 0, 1, 2], [2, 0, 3, 1], [3, 0, 2, 1], // 8-11
    [1, 2, 0, 3], [1, 3, 0, 2], [2, 1, 0, 3], [3, 1, 0, 2], // 12-15
    [2, 3, 0, 1], [3, 2, 0, 1], [1, 2, 3, 0], [1, 3, 2, 0], // 16-19
    [2, 1, 3, 0], [3, 1, 2, 0], [2, 3, 1, 0], [3, 2, 1, 0], // 20-23
];

/// Parsed Pokemon data from memory.
#[derive(Default, Clone)]
struct Pokemon {
    /// Whether this slot has a valid Pokemon.
    valid: bool,
    /// Species ID (internal index).
    species: u16,
    /// Personality value (needed for encryption).
    personality: u32,
    /// Original Trainer ID (needed for encryption).
    ot_id: u32,
    /// Nickname (decoded from GBA text).
    nickname: String,
    /// Level.
    level: u8,
    /// Current HP.
    current_hp: u16,
    /// Max HP.
    max_hp: u16,
}

impl Pokemon {
    /// Parse a Pokemon from raw memory bytes (100 bytes).
    fn from_bytes(data: &[u8]) -> Self {
        if data.len() < POKEMON_SIZE_BYTES {
            return Self::default();
        }

        let personality = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let ot_id = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

        let nickname = decode_gba_text(&data[8..18]);

        let valid = personality != 0 || !nickname.is_empty();

        let level = data[0x54];
        let current_hp = u16::from_le_bytes([data[0x56], data[0x57]]);
        let max_hp = u16::from_le_bytes([data[0x58], data[0x59]]);

        // Decrypt the Growth substructure to get species
        let species = Self::decrypt_species(data, personality, ot_id);

        Self {
            valid: valid && level > 0 && level <= 100,
            species,
            personality,
            ot_id,
            nickname,
            level,
            current_hp,
            max_hp,
        }
    }

    /// Decrypt the Growth substructure and return the species.
    fn decrypt_species(data: &[u8], personality: u32, ot_id: u32) -> u16 {
        let key = personality ^ ot_id;
        let order_idx = (personality % 24) as usize;

        // Find where Growth substructure (index 0) is located
        let order = &SUBSTRUCTURE_ORDERS[order_idx];
        let growth_pos = order.iter().position(|&x| x == 0).unwrap_or(0);
        let growth_offset = 0x20 + growth_pos * 12;

        if growth_offset + 4 > data.len() {
            return 0;
        }

        // Decrypt the first word of Growth substructure
        // Growth layout: [species:u16, item:u16] [experience:u32] [pp_bonuses:u8, friendship:u8, unknown:u16]
        let word0 = u32::from_le_bytes([
            data[growth_offset],
            data[growth_offset + 1],
            data[growth_offset + 2],
            data[growth_offset + 3],
        ]) ^ key;

        let word0_bytes = word0.to_le_bytes();
        u16::from_le_bytes([word0_bytes[0], word0_bytes[1]])
    }

    /// Calculate the experience needed for a given level (Medium Fast growth rate).
    const fn exp_for_level(level: u8) -> u32 {
        if level == 0 || level > 100 {
            return 0;
        }
        MEDIUM_FAST_EXP[(level - 1) as usize]
    }

    /// Get the species name.
    /// Gen 3 stores species by internal index, which must be converted to National Dex.
    fn species_name(&self) -> &'static str {
        // Convert internal species index to National Dex number
        let national_dex = if (self.species as usize) < SPECIES_TO_NATIONAL.len() {
            SPECIES_TO_NATIONAL[self.species as usize] as usize
        } else {
            0
        };

        if national_dex < SPECIES_NAMES.len() {
            SPECIES_NAMES[national_dex]
        } else {
            "???"
        }
    }
}

/// Decode GBA text encoding to UTF-8 string.
fn decode_gba_text(data: &[u8]) -> String {
    let mut result = String::new();
    for &byte in data {
        let c = match byte {
            0xFF => break, // Terminator
            0x00 => ' ',
            0xBB => 'A',
            0xBC => 'B',
            0xBD => 'C',
            0xBE => 'D',
            0xBF => 'E',
            0xC0 => 'F',
            0xC1 => 'G',
            0xC2 => 'H',
            0xC3 => 'I',
            0xC4 => 'J',
            0xC5 => 'K',
            0xC6 => 'L',
            0xC7 => 'M',
            0xC8 => 'N',
            0xC9 => 'O',
            0xCA => 'P',
            0xCB => 'Q',
            0xCC => 'R',
            0xCD => 'S',
            0xCE => 'T',
            0xCF => 'U',
            0xD0 => 'V',
            0xD1 => 'W',
            0xD2 => 'X',
            0xD3 => 'Y',
            0xD4 => 'Z',
            0xD5 => 'a',
            0xD6 => 'b',
            0xD7 => 'c',
            0xD8 => 'd',
            0xD9 => 'e',
            0xDA => 'f',
            0xDB => 'g',
            0xDC => 'h',
            0xDD => 'i',
            0xDE => 'j',
            0xDF => 'k',
            0xE0 => 'l',
            0xE1 => 'm',
            0xE2 => 'n',
            0xE3 => 'o',
            0xE4 => 'p',
            0xE5 => 'q',
            0xE6 => 'r',
            0xE7 => 's',
            0xE8 => 't',
            0xE9 => 'u',
            0xEA => 'v',
            0xEB => 'w',
            0xEC => 'x',
            0xED => 'y',
            0xEE => 'z',
            0xA1..=0xAA => (b'0' + (byte - 0xA1)) as char, // 0-9
            _ => '?',
        };
        result.push(c);
    }
    result
}

/// Debug tool for viewing and editing Pokemon game data.
pub struct PokemonDebugger {
    emu_handle: Arc<Mutex<EmuHandle>>,
    /// Cached party data.
    party_data: Vec<u8>,
    /// Parsed party Pokemon.
    party: [Pokemon; PARTY_SIZE],
    /// Whether we're waiting for memory data.
    pending_request: bool,
    /// Auto-refresh enabled.
    auto_refresh: bool,
    /// Frames since last refresh.
    refresh_counter: u32,
    /// Currently selected tab.
    selected_tab: Tab,
    /// Currently selected game version.
    game_version: GameVersion,
    /// Address that the pending request was made to.
    pending_address: u32,
    /// Debug: last received data info.
    debug_info: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Party,
}

impl PokemonDebugger {
    pub fn new(emu_handle: Arc<Mutex<EmuHandle>>) -> Self {
        // Auto-detect game version from game code
        let game_version = emu_handle
            .lock()
            .map(|h| GameVersion::from_game_code(&h.state.game_code))
            .unwrap_or(GameVersion::Unknown);

        Self {
            emu_handle,
            party_data: vec![0; PARTY_BYTES],
            party: std::array::from_fn(|_| Pokemon::default()),
            pending_request: false,
            auto_refresh: true,
            refresh_counter: 0,
            selected_tab: Tab::Party,
            game_version,
            pending_address: game_version.party_address(),
            debug_info: format!("Auto-detected: {}", game_version.display_name()),
        }
    }

    /// Get the effective party address based on game version selection.
    const fn effective_address(&self) -> u32 {
        self.game_version.party_address()
    }

    fn request_party_data(&mut self) {
        let address = self.effective_address();
        if let Ok(mut handle) = self.emu_handle.lock() {
            handle.send(EmuCommand::ReadMemory {
                address,
                length: PARTY_BYTES,
            });
            self.pending_request = true;
            self.pending_address = address;
            self.debug_info = format!("Requested 0x{address:08X}");
        }
    }

    fn check_pending_data(&mut self) {
        let received_data = if let Ok(mut handle) = self.emu_handle.lock() {
            if let Some((addr, data)) = handle.pending_memory_data.take() {
                self.debug_info = format!(
                    "Got data: addr=0x{:08X}, len={}, expected=0x{:08X}",
                    addr,
                    data.len(),
                    self.pending_address
                );
                if addr == self.pending_address && data.len() >= PARTY_BYTES {
                    Some(data)
                } else {
                    // Put it back if it's not for us
                    handle.pending_memory_data = Some((addr, data));
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(data) = received_data {
            self.party_data.clone_from(&data);
            self.parse_party();
            self.pending_request = false;
            self.debug_info.clear();
        }
    }

    fn parse_party(&mut self) {
        for i in 0..PARTY_SIZE {
            let start = i * POKEMON_SIZE_BYTES;
            let end = start + POKEMON_SIZE_BYTES;
            if end <= self.party_data.len() {
                self.party[i] = Pokemon::from_bytes(&self.party_data[start..end]);
            }
        }
    }

    /// Write a byte to Pokemon memory.
    fn write_byte(&self, slot: u32, offset: u32, value: u8) {
        let address = self.effective_address() + slot * POKEMON_SIZE + offset;
        if let Ok(mut handle) = self.emu_handle.lock() {
            handle.send(EmuCommand::WriteByte { address, value });
        }
    }

    /// Write a 16-bit value to Pokemon memory (little-endian).
    fn write_u16(&self, slot: u32, offset: u32, value: u16) {
        let value_bytes = value.to_le_bytes();
        self.write_byte(slot, offset, value_bytes[0]);
        self.write_byte(slot, offset + 1, value_bytes[1]);
    }

    /// Quick cheat: Max out current HP.
    fn heal_pokemon(&self, slot: u32) {
        let pokemon = &self.party[slot as usize];
        if pokemon.valid {
            self.write_u16(slot, 0x56, pokemon.max_hp);
        }
    }

    /// Quick cheat: Increment level by 1.
    /// This properly modifies the encrypted experience value and recalculates the checksum.
    fn level_up(&mut self, slot: u32) {
        let slot_usize = slot as usize;
        let pokemon = &self.party[slot_usize];
        if !pokemon.valid || pokemon.level >= 100 {
            self.debug_info = "Invalid pokemon or max level".to_string();
            return;
        }

        let new_level = pokemon.level + 1;
        let new_exp = Pokemon::exp_for_level(new_level);
        let old_level = pokemon.level;

        // Get the raw Pokemon data for this slot
        let start = slot_usize * POKEMON_SIZE_BYTES;
        let end = start + POKEMON_SIZE_BYTES;
        if end > self.party_data.len() {
            self.debug_info = "Party data too short".to_string();
            return;
        }

        let data = &self.party_data[start..end];
        let personality = pokemon.personality;
        let ot_id = pokemon.ot_id;
        let key = personality ^ ot_id;
        let order_idx = (personality % 24) as usize;

        // Read the stored checksum for verification
        let stored_checksum = u16::from_le_bytes([data[0x1C], data[0x1D]]);

        // Find Growth substructure position
        let order = &SUBSTRUCTURE_ORDERS[order_idx];
        let growth_pos = order.iter().position(|&x| x == 0).unwrap_or(0);
        let growth_start = growth_pos * 12;

        // Decrypt all 48 bytes of substructure data
        let mut decrypted = [0u8; 48];
        for i in 0..12 {
            let offset = 0x20 + i * 4;
            let encrypted = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let dec = encrypted ^ key;
            decrypted[i * 4..i * 4 + 4].copy_from_slice(&dec.to_le_bytes());
        }

        // Read old experience for debug
        let old_exp = u32::from_le_bytes([
            decrypted[growth_start + 4],
            decrypted[growth_start + 5],
            decrypted[growth_start + 6],
            decrypted[growth_start + 7],
        ]);

        // Calculate OLD checksum before modifying (should match stored_checksum)
        let mut old_calc_checksum: u16 = 0;
        for i in 0..24 {
            let word = u16::from_le_bytes([decrypted[i * 2], decrypted[i * 2 + 1]]);
            old_calc_checksum = old_calc_checksum.wrapping_add(word);
        }

        // Modify experience in Growth substructure (bytes 4-7 within Growth)
        decrypted[growth_start + 4..growth_start + 8].copy_from_slice(&new_exp.to_le_bytes());

        // Calculate NEW checksum (sum of all decrypted u16 values)
        let mut new_checksum: u16 = 0;
        for i in 0..24 {
            let word = u16::from_le_bytes([decrypted[i * 2], decrypted[i * 2 + 1]]);
            new_checksum = new_checksum.wrapping_add(word);
        }

        // Re-encrypt the data
        let mut encrypted = [0u8; 48];
        for i in 0..12 {
            let dec_word = u32::from_le_bytes([
                decrypted[i * 4],
                decrypted[i * 4 + 1],
                decrypted[i * 4 + 2],
                decrypted[i * 4 + 3],
            ]);
            let enc = dec_word ^ key;
            encrypted[i * 4..i * 4 + 4].copy_from_slice(&enc.to_le_bytes());
        }

        // Write the checksum (offset 0x1C in Pokemon structure)
        let checksum_bytes = new_checksum.to_le_bytes();
        self.write_byte(slot, 0x1C, checksum_bytes[0]);
        self.write_byte(slot, 0x1D, checksum_bytes[1]);

        // Write the encrypted data (offset 0x20, 48 bytes)
        for (offset, &byte) in (0u32..).zip(encrypted.iter()) {
            self.write_byte(slot, 0x20 + offset, byte);
        }

        // Also update the cached level in battle stats (0x54)
        // The game will recalculate this, but updating it helps see immediate feedback
        self.write_byte(slot, 0x54, new_level);

        // Debug: stored should equal old_calc if our decryption is correct
        let checksum_ok = if stored_checksum == old_calc_checksum {
            "OK"
        } else {
            "MISMATCH!"
        };
        self.debug_info = format!(
            "Lv{old_level}->{new_level} exp:{old_exp}->{new_exp} chk:{stored_checksum:04X}=={old_calc_checksum:04X}? {checksum_ok} new:{new_checksum:04X}"
        );
    }

    fn show_party_tab(&mut self, ui: &mut egui::Ui) {
        // Clone party data to avoid borrow issues
        let party_clone: Vec<Pokemon> = self.party.to_vec();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (slot, pokemon) in (0u32..).zip(party_clone.iter()) {
                if !pokemon.valid {
                    ui.weak(format!("Slot {}: (empty)", slot + 1));
                    continue;
                }

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(format!("{}.", slot + 1));
                        ui.label(pokemon.species_name());
                        if !pokemon.nickname.is_empty()
                            && pokemon.nickname != pokemon.species_name()
                        {
                            ui.weak(format!("\"{}\"", pokemon.nickname));
                        }
                        ui.label(format!("Lv.{}", pokemon.level));
                    });

                    ui.horizontal(|ui| {
                        // HP bar
                        let hp_ratio = if pokemon.max_hp > 0 {
                            f32::from(pokemon.current_hp) / f32::from(pokemon.max_hp)
                        } else {
                            0.0
                        };
                        let hp_color = if hp_ratio > 0.5 {
                            egui::Color32::GREEN
                        } else if hp_ratio > 0.2 {
                            egui::Color32::YELLOW
                        } else {
                            egui::Color32::RED
                        };

                        ui.label("HP:");
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(80.0, 10.0), egui::Sense::hover());
                        ui.painter()
                            .rect_filled(rect, 2.0, egui::Color32::DARK_GRAY);
                        let filled_rect = egui::Rect::from_min_size(
                            rect.min,
                            egui::vec2(rect.width() * hp_ratio, rect.height()),
                        );
                        ui.painter().rect_filled(filled_rect, 2.0, hp_color);
                        ui.label(format!("{}/{}", pokemon.current_hp, pokemon.max_hp));
                    });

                    // Cheat buttons
                    ui.horizontal(|ui| {
                        if ui.button("Heal").clicked() {
                            self.heal_pokemon(slot);
                            self.request_party_data();
                        }
                        if ui.button("Lv+").clicked() {
                            self.level_up(slot);
                            self.request_party_data();
                        }
                    });
                });

                ui.add_space(2.0);
            }
        });
    }
}

impl UiTool for PokemonDebugger {
    fn name(&self) -> &'static str {
        "Pokemon Debugger"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        self.check_pending_data();

        if self.auto_refresh {
            self.refresh_counter += 1;
            if self.refresh_counter >= 30 && !self.pending_request {
                self.refresh_counter = 0;
                self.request_party_data();
            }
        }

        egui::Window::new(self.name())
            .open(open)
            .default_width(350.0)
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Game:");
            egui::ComboBox::from_id_salt("game_version")
                .selected_text(self.game_version.display_name())
                .show_ui(ui, |ui| {
                    for &version in GameVersion::ALL {
                        if ui
                            .selectable_value(
                                &mut self.game_version,
                                version,
                                version.display_name(),
                            )
                            .changed()
                        {
                            // Clear data and refresh when version changes
                            self.party = std::array::from_fn(|_| Pokemon::default());
                            self.request_party_data();
                        }
                    }
                });
        });

        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                self.request_party_data();
            }
            ui.checkbox(&mut self.auto_refresh, "Auto");
            if self.pending_request {
                ui.spinner();
            }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.selected_tab, Tab::Party, "Party");
        });

        ui.separator();

        match self.selected_tab {
            Tab::Party => self.show_party_tab(ui),
        }

        ui.separator();
        let addr = self.effective_address();
        ui.weak(format!("Reading from: 0x{addr:08X}"));

        if !self.debug_info.is_empty() {
            ui.weak(&self.debug_info);
        }
    }
}
