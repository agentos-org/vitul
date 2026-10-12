#include <windows.h>

int WINAPI WinMain(HINSTANCE instance, HINSTANCE previous, LPSTR command_line, int show_command) {
  (void)instance;
  (void)previous;
  (void)command_line;
  (void)show_command;

  MessageBoxA(
      NULL,
      "Hello from WinTone! This is a minimal Windows GUI app.",
      "WinTone Windows Hello",
      MB_OK | MB_ICONINFORMATION);
  return 0;
}
