use crate::engine::scanner::PlayerDef;

pub fn builtin_player_registry() -> Vec<PlayerDef> {
    vec![
        PlayerDef {
            process_name: "gom.exe".to_string(),
            window_title_hint: Some("GOM Player".to_string()),
        },
        PlayerDef {
            process_name: "kmplayer.exe".to_string(),
            window_title_hint: Some("KMPlayer".to_string()),
        },
        PlayerDef {
            process_name: "kodi.exe".to_string(),
            window_title_hint: Some("Kodi".to_string()),
        },
        PlayerDef {
            process_name: "mpc-be64.exe".to_string(),
            window_title_hint: Some("Media Player Classic".to_string()),
        },
        PlayerDef {
            process_name: "mpc-hc.exe".to_string(),
            window_title_hint: Some("Media Player Classic".to_string()),
        },
        PlayerDef {
            process_name: "mpc-hc64.exe".to_string(),
            window_title_hint: Some("Media Player Classic".to_string()),
        },
        PlayerDef {
            process_name: "mplayer2.exe".to_string(),
            window_title_hint: Some("MPlayer".to_string()),
        },
        PlayerDef {
            process_name: "mpv.exe".to_string(),
            window_title_hint: Some("mpv".to_string()),
        },
        PlayerDef {
            process_name: "mpv.net.exe".to_string(),
            window_title_hint: Some("mpv.net".to_string()),
        },
        PlayerDef {
            process_name: "potplayer.exe".to_string(),
            window_title_hint: Some("PotPlayer".to_string()),
        },
        PlayerDef {
            process_name: "smplayer.exe".to_string(),
            window_title_hint: Some("SMPlayer".to_string()),
        },
        PlayerDef {
            process_name: "vlc.exe".to_string(),
            window_title_hint: Some("VLC".to_string()),
        },
        PlayerDef {
            process_name: "wmplayer.exe".to_string(),
            window_title_hint: Some("Windows Media Player".to_string()),
        },
    ]
}
