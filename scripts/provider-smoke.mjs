import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const REPORT_SCHEMA_VERSION = 1;
const COMMAND_TIMEOUT_MS = 300_000;
const VERSION_TIMEOUT_MS = 10_000;
const FAILURE_OUTPUT_LIMIT = 4_000;
const DEFAULT_OUTPUT = "target/aurapilot/provider-compatibility.json";
const CONFIRM_FLAG = "--confirm-real-provider";

const cases = {
  codex: {
    caseId: "codex-existing-thread-read",
    executable: () => "codex",
    requiredEnvironment: ["AURAPILOT_CODEX_THREAD_ID"],
    cargoArgs: [
      "test",
      "-p",
      "aurapilot-codex",
      "--locked",
      "tests::verifies_a_real_thread_through_the_managed_websocket",
      "--",
      "--ignored",
      "--exact",
      "--nocapture",
    ],
    environment: () => ({}),
  },
  opencode: {
    caseId: "opencode-empty-session-lifecycle",
    executable: () => process.env.AURAPILOT_OPENCODE_BIN || "opencode",
    requiredEnvironment: [],
    cargoArgs: [
      "test",
      "-p",
      "aurapilot",
      "--lib",
      "--locked",
      "providers::opencode::tests::real_server_creates_and_verifies_a_session_without_sending_a_prompt",
      "--",
      "--ignored",
      "--exact",
      "--nocapture",
    ],
    environment: (executable) => ({ AURAPILOT_OPENCODE_BIN: executable }),
  },
};

function usage() {
  return `AuraPilot real Provider smoke (Beta)

Usage:
  node scripts/provider-smoke.mjs --provider codex|opencode|all ${CONFIRM_FLAG} [--output FILE]

The confirmation flag is mandatory. These checks use installed Provider CLIs and
existing local authentication. They do not send a task or Pointer Prompt.

Codex requires AURAPILOT_CODEX_THREAD_ID and only reads that existing Thread.

OpenCode uses AURAPILOT_OPENCODE_BIN when set, otherwise "opencode" from PATH.
It starts a local Server and creates, verifies, forks, and aborts empty Sessions.

The JSON report defaults to ${DEFAULT_OUTPUT}.`;
}

function parseArguments(argv) {
  let provider;
  let output = DEFAULT_OUTPUT;
  let confirmed = false;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--") {
      continue;
    }
    if (argument === "-h" || argument === "--help") {
      return { help: true };
    }
    if (argument === CONFIRM_FLAG) {
      confirmed = true;
      continue;
    }
    if (argument === "--provider") {
      provider = argv[index + 1];
      index += 1;
      continue;
    }
    if (argument === "--output") {
      output = argv[index + 1];
      index += 1;
      continue;
    }
    throw new Error(`unknown option: ${argument}`);
  }
  if (!provider || !["codex", "opencode", "all"].includes(provider)) {
    throw new Error("--provider must be codex, opencode, or all");
  }
  if (!output) {
    throw new Error("--output requires a file path");
  }
  if (!confirmed) {
    throw new Error(`refusing to use real Provider CLIs without ${CONFIRM_FLAG}`);
  }
  return { help: false, provider, output };
}

function commandVersion(executable) {
  const result = spawnSync(executable, ["--version"], {
    encoding: "utf8",
    timeout: VERSION_TIMEOUT_MS,
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    return {
      available: false,
      version: null,
      failure: result.error?.message || bounded(result.stderr || result.stdout || "version command failed"),
    };
  }
  return {
    available: true,
    version: firstNonEmptyLine(result.stdout || result.stderr) || "version not reported",
    failure: null,
  };
}

function runCase(provider) {
  const definition = cases[provider];
  const startedAt = new Date();
  const executable = definition.executable();
  const missingEnvironment = definition.requiredEnvironment.filter(
    (name) => !process.env[name],
  );
  if (missingEnvironment.length > 0) {
    return resultRecord({
      definition,
      provider,
      executable,
      version: null,
      startedAt,
      status: "configuration_error",
      failureReason: `missing environment: ${missingEnvironment.join(", ")}`,
    });
  }
  const version = commandVersion(executable);
  if (!version.available) {
    const reasons = [];
    reasons.push(`Provider executable unavailable: ${version.failure}`);
    return resultRecord({
      definition,
      provider,
      executable,
      version: version.version,
      startedAt,
      status: "configuration_error",
      failureReason: reasons.join("; "),
    });
  }

  const result = spawnSync("cargo", definition.cargoArgs, {
    cwd: resolve(import.meta.dirname, ".."),
    encoding: "utf8",
    env: { ...process.env, ...definition.environment(executable) },
    timeout: COMMAND_TIMEOUT_MS,
    maxBuffer: 8 * 1024 * 1024,
    windowsHide: true,
  });
  const timedOut = result.error?.code === "ETIMEDOUT";
  const passed = !result.error && result.status === 0;
  return resultRecord({
    definition,
    provider,
    executable,
    version: version.version,
    startedAt,
    status: passed ? "passed" : timedOut ? "timed_out" : "failed",
    failureReason: passed
      ? null
      : result.error?.message || `cargo test exited with status ${result.status}`,
    failureOutput: passed ? null : bounded(`${result.stdout || ""}\n${result.stderr || ""}`),
  });
}

function resultRecord({
  definition,
  provider,
  executable,
  version,
  startedAt,
  status,
  failureReason,
  failureOutput = null,
}) {
  return {
    case_id: definition.caseId,
    provider,
    evidence_level: "real_provider",
    status,
    provider_executable: executable,
    provider_version: version,
    started_at: startedAt.toISOString(),
    completed_at: new Date().toISOString(),
    duration_ms: Date.now() - startedAt.getTime(),
    prompt_sent: false,
    failure_reason: failureReason,
    failure_output: failureOutput,
  };
}

function firstNonEmptyLine(value) {
  return value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean);
}

function bounded(value) {
  const trimmed = value.trim();
  return trimmed.length <= FAILURE_OUTPUT_LIMIT
    ? trimmed
    : `${trimmed.slice(0, FAILURE_OUTPUT_LIMIT)}…`;
}

function writeReport(output, selectedProviders, results, startedAt) {
  const counts = Object.fromEntries(
    ["passed", "failed", "timed_out", "configuration_error"].map((status) => [
      status,
      results.filter((result) => result.status === status).length,
    ]),
  );
  const report = {
    schema_version: REPORT_SCHEMA_VERSION,
    suite: "aurapilot-provider-smoke",
    evidence_level: "real_provider",
    beta: true,
    selected_providers: selectedProviders,
    started_at: startedAt.toISOString(),
    completed_at: new Date().toISOString(),
    host: { platform: process.platform, architecture: process.arch },
    safety: {
      explicit_confirmation: true,
      prompt_sent: false,
      ordinary_ci: false,
    },
    summary: { total: results.length, ...counts },
    results,
  };
  const destination = resolve(output);
  mkdirSync(dirname(destination), { recursive: true });
  writeFileSync(destination, `${JSON.stringify(report, null, 2)}\n`, { flag: "w" });
  return destination;
}

let options;
try {
  options = parseArguments(process.argv.slice(2));
} catch (error) {
  console.error(`error: ${error.message}\n\n${usage()}`);
  process.exitCode = 2;
}

if (options?.help) {
  console.log(usage());
} else if (options) {
  const startedAt = new Date();
  const selectedProviders = options.provider === "all" ? Object.keys(cases) : [options.provider];
  const results = selectedProviders.map(runCase);
  const destination = writeReport(options.output, selectedProviders, results, startedAt);
  for (const result of results) {
    console.log(`${result.provider}: ${result.status} (${result.case_id})`);
    if (result.failure_reason) console.log(`  ${result.failure_reason}`);
  }
  console.log(`Report: ${destination}`);
  if (results.some((result) => result.status !== "passed")) process.exitCode = 1;
}
