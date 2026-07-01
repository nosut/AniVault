import { TrayIcon } from '@tauri-apps/api/tray';
import { Menu } from '@tauri-apps/api/menu';
import { invoke } from '@tauri-apps/api/core';

export async function setupTray(): Promise<void> {
  const menu = await Menu.new({
    items: [
      {
        id: 'show-anivault',
        text: 'Show AniVault',
        action: () => {
          /* window is already shown; tray menu closes automatically */
        },
      },
      {
        id: 'quit',
        text: 'Quit',
        action: async () => {
          await invoke('app_exit');
        },
      },
    ],
  });

  await TrayIcon.new({ menu, menuOnLeftClick: false });
}
