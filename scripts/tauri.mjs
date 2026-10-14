import { spawn } from "node:child_process";

const args = process.argv.slice(2);
if (args[0] === "dev" && !args.includes("--config") && !args.includes("-c")) {
  args.splice(1, 0, "--config", "src-tauri/tauri.dev.conf.json");
}

const child = spawn("tauri", args, { stdio: "inherit" });
child.on("exit", (code, signal) => {
  if (signal) process.kill(process.pid, signal);
  process.exit(code ?? 1);
});
