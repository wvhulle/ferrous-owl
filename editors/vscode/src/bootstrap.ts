import fs from "node:fs/promises";
import { spawn, spawnSync } from "node:child_process";
import * as vscode from "vscode";
import packageJson from "../package.json";

const version: string = packageJson.version;

export const hostTuple = (): string | null => {
  let arch = null;
  if (process.arch === "arm64") {
    arch = "aarch64";
  } else if (process.arch === "x64") {
    arch = "x86_64";
  }
  let platform = null;
  if (process.platform === "linux") {
    platform = "unknown-linux-gnu";
  } else if (process.platform === "darwin") {
    platform = "apple-darwin";
  } else if (process.platform === "win32") {
    platform = "pc-windows-msvc";
  }
  if (arch && platform) {
    return `${arch}-${platform}`;
  } else {
    return null;
  }
};

const exeExt = hostTuple()?.includes("windows") ? ".exe" : "";

interface RustowlConfig {
  serverPath: string;
  skipToolchainSetup: boolean;
}

const getConfig = (): RustowlConfig => {
  const config = vscode.workspace.getConfiguration("rustowl");
  return {
    serverPath: config.get<string>("serverPath", ""),
    skipToolchainSetup: config.get<boolean>("skipToolchainSetup", false),
  };
};

export const downloadRustowl = async (basePath: string) => {
  const baseUrl = `https://github.com/cordx56/rustowl/releases/download/v${version}`;
  const host = hostTuple();
  if (host) {
    const owl = await fetch(`${baseUrl}/rustowl-${host}${exeExt}`);
    if (owl.status !== 200) {
      throw Error("RustOwl download error");
    }
    await fs.writeFile(
      `${basePath}/rustowl${exeExt}`,
      Buffer.from(await owl.arrayBuffer()),
      { flag: "w" },
    );
    await fs.chmod(`${basePath}/rustowl${exeExt}`, "755");
  } else {
    throw Error("unsupported architecture or platform");
  }
};

const exists = async (path: string) => {
  return fs
    .access(path)
    .then(() => true)
    .catch(() => false);
};
export const needUpdated = async (currentVersion: string) => {
  if (!currentVersion) {
    return true;
  }
  console.log(`current RustOwl version: ${currentVersion.trim()}`);
  console.log(`extension version: v${version}`);
  try {
    const semverParser = await import("semver-parser");
    const current = semverParser.parseSemVer(currentVersion.trim(), false);
    const self = semverParser.parseSemVer(version, false);
    if (
      current.major === self.major &&
      current.minor === self.minor &&
      current.patch === self.patch &&
      JSON.stringify(current.pre) === JSON.stringify(self.pre)
    ) {
      return false;
    } else {
      return true;
    }
  } catch (_e) {
    return true;
  }
};
const getRustowlCommand = async (dirPath: string) => {
  const rustowlPath = `${dirPath}/rustowl${exeExt}`;
  if (spawnSync("rustowl", ["--version", "--quiet"]).stdout?.toString()) {
    return "rustowl";
  } else if (
    (await exists(rustowlPath)) &&
    spawnSync(rustowlPath, ["--version", "--quiet"]).stdout?.toString()
  ) {
    return rustowlPath;
  } else {
    return null;
  }
};

export const bootstrapRustowl = async (dirPath: string): Promise<string> => {
  const config = getConfig();

  // If serverPath is configured, use it directly without any setup
  if (config.serverPath) {
    const serverPath = config.serverPath;
    const result = spawnSync(serverPath, ["--version", "--quiet"]);
    if (result.stdout?.toString()) {
      console.log(`Using configured serverPath: ${serverPath}`);
      return serverPath;
    } else {
      throw Error(
        `Configured serverPath "${serverPath}" is not a valid rustowl executable`,
      );
    }
  }

  let rustowlCommand = await getRustowlCommand(dirPath);

  // If skipToolchainSetup is enabled and we have a working rustowl, use it directly
  if (config.skipToolchainSetup && rustowlCommand) {
    console.log(
      `Using rustowl without toolchain setup (skipToolchainSetup=true): ${rustowlCommand}`,
    );
    return rustowlCommand;
  }

  if (
    !rustowlCommand ||
    (await needUpdated(
      spawnSync(rustowlCommand, ["--version", "--quiet"]).stdout?.toString(),
    ))
  ) {
    await fs.mkdir(dirPath, { recursive: true });
    // download rustowl binary
    await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: "RustOwl installing",
        cancellable: false,
      },
      async (progress) => {
        try {
          progress.report({ message: "binary downloading" });
          await downloadRustowl(dirPath);
          progress.report({ message: "binary downloaded" });
        } catch (e) {
          vscode.window.showErrorMessage(`${e}`);
        }
      },
    );

    // Skip toolchain setup if configured
    if (config.skipToolchainSetup) {
      rustowlCommand = await getRustowlCommand(dirPath);
      if (!rustowlCommand) {
        throw Error("failed to install RustOwl");
      }
      console.log(
        `Skipping toolchain setup (skipToolchainSetup=true): ${rustowlCommand}`,
      );
      return rustowlCommand;
    }

    rustowlCommand = await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: "Setup RustOwl toolchain",
        cancellable: false,
      },
      async (progress) => {
        try {
          rustowlCommand = await getRustowlCommand(dirPath);

          if (!rustowlCommand) {
            throw Error("failed to install RustOwl");
          }

          const installer = spawn(rustowlCommand, ["toolchain", "install"], {
            stdio: ["ignore", "ignore", "pipe"],
          });
          installer.stderr.addListener("data", (data) => {
            if (`${data}`.includes("%")) {
              progress.report({
                message: "toolchain downloading",
                increment: 0.25, // downloads 4 toolchain components
              });
            }
          });
          return new Promise((resolve, reject) => {
            installer.addListener("exit", (code) => {
              if (code === 0) {
                resolve(rustowlCommand);
              } else {
                reject(`toolchain setup failed (exit code ${code})`);
              }
            });
          });
        } catch (e) {
          vscode.window.showErrorMessage(`${e}`);
        }
        return null;
      },
    );
  }

  if (!rustowlCommand) {
    throw Error("failed to install RustOwl");
  }

  return rustowlCommand;
};
