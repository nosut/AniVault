import { TrayIcon } from '@tauri-apps/api/tray';
import { Menu } from '@tauri-apps/api/menu';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

export async function setupTray(): Promise<void> {
  const menu = await Menu.new({
    items: [
      {
        id: 'show-anivault',
        text: 'Show AniVault',
        action: async () => {
          const win = getCurrentWindow();
          await win.show();
          await win.setFocus();
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
