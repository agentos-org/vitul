import { callOpenTool } from "./opentool";
import { createHostProvider } from "./host";

const hostStatus = document.querySelector<HTMLElement>("#host-status");
const systemMessage = document.querySelector<HTMLElement>("#system-message");
const minimizeButton = document.querySelector<HTMLButtonElement>("#minimize-button");
const form = document.querySelector<HTMLFormElement>("#greet-form");
const input = document.querySelector<HTMLInputElement>("#greet-input");
const message = document.querySelector<HTMLElement>("#greet-msg");

const host = await createHostProvider();
if (hostStatus) hostStatus.textContent = `Connected through the ${host.platform} host.`;

minimizeButton?.addEventListener("click", async () => {
  if (!systemMessage) return;
  systemMessage.textContent = "Sending window.minimize through host IPC…";
  try {
    await host.window.minimize();
    systemMessage.textContent = "Window minimized by the native host.";
  } catch (error) {
    systemMessage.textContent = error instanceof Error ? error.message : String(error);
  }
});

form?.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!input || !message) return;
  message.textContent = "Calling OpenTool…";
  try {
    const result = await callOpenTool<{ message: string }>("greet", { name: input.value });
    message.textContent = result.message;
  } catch (error) {
    message.textContent = error instanceof Error ? error.message : String(error);
  }
});
