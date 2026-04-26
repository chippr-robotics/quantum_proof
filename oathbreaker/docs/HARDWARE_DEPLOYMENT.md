# Hardware Deployment -- Reproducing the Run

This guide explains how anyone can clone this repository and submit the
same Oathbreaker circuits to real IBM Quantum hardware, with every input
and output preserved as a verifiable artefact.

The pipeline is identical to the one the maintainer runs from the
Actions tab of the canonical repository. The same workflow file lives
in `.github/workflows/quantum-deploy.yml`; the only thing you need to
change is the IBM Quantum API key.

## What gets preserved

Every dispatch of the workflow uploads a single archive of evidence:

| File                                           | Why it matters                                                                                              |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `manifest.json`                                | Commit SHA, ref, dispatcher, timestamp, all inputs, IBM job id, and the SHA-256 of the QASM circuit.        |
| `oathbreaker_oath{N}.qasm`                     | The exact OpenQASM 3.0 circuit submitted (Oath-8/16/32/64). Identical to the file `cargo` produces locally. |
| `oathbreaker_oath{N}.stats.json`               | Machine-readable resource summary -- qubits, Toffoli, gate totals.                                          |
| `runner.log`                                   | Full stdout/stderr of the runner: backend selection, transpile depth, two-qubit count, and IBM job id.      |
| `ibm_job_id.txt`                               | The job id alone, on its own line, easy to paste into the IBM Quantum dashboard.                            |

Artifacts are retained for 90 days. The IBM job id remains queryable
from `quantum.ibm.com` for the lifetime of your IBM account.

## What you need

1. **A clone of this repository.** No fork required if you only want to
   read; a fork is required to run the workflow on your own credits.

   ```bash
   git clone https://github.com/chippr-robotics/quantum_proof.git
   cd quantum_proof
   ```

2. **An IBM Quantum Platform account.** Sign up at
   [quantum.ibm.com](https://quantum.ibm.com); the Open Plan gives you
   free monthly time on Heron and Eagle backends, which is enough to
   run the Oath-4 POC.

3. **Your API key.** Generate one from your account settings on
   `quantum.ibm.com`. Treat it as a secret -- it bills against your
   account.

4. **A GitHub repository secret named `IBM_QUANTUM_API`.** In your fork:
   *Settings -> Secrets and variables -> Actions -> New repository
   secret*. Name: `IBM_QUANTUM_API`. Value: your IBM Quantum API key.

   The workflow re-exports this as `IBM_QUANTUM_TOKEN` inside the
   runner so the existing Python scripts read it without modification.

## Running it

1. Open the **Actions** tab in your fork.

2. Pick the **Quantum Hardware Deployment** workflow.

3. Click **Run workflow**. You will see these inputs:

   | Input                  | Meaning                                                                                                   |
   | ---------------------- | --------------------------------------------------------------------------------------------------------- |
   | `tier`                 | Oath tier. `4` runs the validated 12-qubit POC; `8`/`16`/`32`/`64` runs the real-path circuits.           |
   | `k`                    | The planted secret scalar. Used only post hoc to compare the recovered value against ground truth.        |
   | `shots`                | Number of shots to request from the backend.                                                              |
   | `backend`              | Optional explicit backend (e.g. `ibm_torino`). Blank picks the least-busy operational device that fits.   |
   | `optimization_level`   | Qiskit transpiler optimisation level (0--3).                                                              |
   | `compiler`             | `qiskit` (default) or `tket` (pytket pipeline).                                                           |
   | `dynamic_decoupling`   | Insert XY-4 DD on idle windows. Oath-4 POC only; ignored for higher tiers.                                |
   | `dry_run`              | Compile and transpile, report the footprint, do not submit. **Default: on.**                              |
   | `acknowledge_billing`  | Required to be checked when `dry_run` is off. Failsafe against accidental wet submissions.                |

4. Click the green **Run workflow** button.

The workflow:

1. **Pre-flight.** Refuses a wet submission unless billing was
   acknowledged.
2. **Compile QASM.** For Oath-8 and above, builds the Rust benchmark
   crate and emits the OpenQASM 3.0 file. (Oath-4 skips this step --
   the POC builds its circuit in Python.)
3. **Hardware run.** Installs Qiskit + qiskit-ibm-runtime, transpiles
   against the live backend, prints the footprint, and either submits
   or stops at the dry-run boundary.
4. **Upload.** Writes the manifest, copies the QASM and stats into the
   results directory, and uploads everything as a workflow artifact.

When the workflow finishes, the **Summary** page shows the IBM job id,
the chosen backend, the transpiled depth and two-qubit count, and the
last 50 lines of the runner log. Download the `hardware-run-oath{N}-...`
artifact from the bottom of the same page for the raw evidence.

## Why dry-run is on by default

The Oathbreaker circuits at Oath-8 and above are well above the
coherence budget of any deployed NISQ device (see
[NISQ_ROADMAP](/docs/nisq-roadmap)). The runner refuses to submit when
the transpiled two-qubit error count would exceed one error event per
shot at the backend's published median 2q error rate. Dry-run captures
the resource footprint of the compiled circuit, which is the only
useful thing to capture today for those tiers.

For the Oath-4 POC, the Heron Torino backend has been validated end to
end -- a wet submission will return a real distribution and the
classical post-processor recovers `k` for every Oath-4 secret in the
noiseless simulator. Hardware fidelity is what determines the recovery
margin in practice.

## Reproducing locally without GitHub Actions

The workflow is just a wrapper around two scripts. Run them yourself
without touching the Actions tab:

```bash
# 1. Build the Rust QASM exporter and emit every tier (8/16/32/64).
cd oathbreaker
cargo run --release -p benchmark -- export-all-qasm

# 2. Set up the Python environment.
cd qiskit/poc
python -m pip install -r requirements.txt
export IBM_QUANTUM_TOKEN="<your-api-key>"

# 3a. Real-path tiers (8/16/32/64): submit the QASM through the runner.
cd ../..
python qiskit/oathN_hardware_runner.py \
    --qasm oathbreaker_oath16.qasm --tier 16 --k 7 --shots 20000 \
    --dry-run    # remove this flag to actually submit

# 3b. Oath-4 POC: the in-Python builder.
python qiskit/poc/run_ibm.py --k 7 --shots 20000 --dry-run
```

The CI workflow exists to make the run *publicly auditable*: anyone can
look at the workflow run page, confirm which commit ran, download the
artefacts, and re-execute the same circuit themselves. None of that
auditability changes if you choose to run locally instead -- but you
will need to publish the artefacts somewhere yourself if you want
others to verify your run.

## Verifying somebody else's run

If you are reviewing a published run rather than producing one:

1. From the workflow run page, download the
   `hardware-run-oath{N}-{run-id}` artifact.
2. Open `manifest.json`. Note the `commit_sha`.
3. Check out that commit locally:

   ```bash
   git fetch origin
   git checkout <commit_sha>
   ```

4. For Oath-8+: rebuild the QASM and confirm its SHA-256 matches
   `circuit.qasm_sha256`:

   ```bash
   cd oathbreaker
   cargo run --release -p benchmark -- export-all-qasm
   sha256sum oathbreaker_oath{N}.qasm
   ```

   The hash must match exactly. If it does, the QASM in the artifact
   is byte-identical to the one this commit emits.

5. Open the IBM Quantum dashboard, find the job by `ibm_job_id`, and
   confirm the submitter, the backend, the shot count, and the raw
   output. The raw output cannot be forged once the job has been
   recorded by IBM's API.

The combination of (commit SHA, QASM SHA-256, IBM job id) is the
verification triple. Two of those values are produced and timestamped
by infrastructure outside the maintainer's control.

## Cost expectation

| Tier   | Time on backend                | Open-plan friendly?              |
| ------ | ------------------------------ | -------------------------------- |
| Oath-4 | seconds, < 5s of Sampler time  | yes (free monthly Heron credits) |
| Oath-8 | refuses to submit on NISQ      | dry-run only                     |
| Oath-16+ | refuses to submit on NISQ    | dry-run only                     |

The runner's coherence gate is intentional: submitting a 112,000-Toffoli
circuit on Eagle only buys you noise. Use dry-run to capture the
transpile footprint; that is the publishable scientific artefact for
those tiers today.

## Where to look in the codebase

| Path                                              | Role                                                       |
| ------------------------------------------------- | ---------------------------------------------------------- |
| `.github/workflows/quantum-deploy.yml`            | The dispatchable workflow.                                 |
| `oathbreaker/qiskit/oathN_hardware_runner.py`     | Real-path runner (loads QASM, transpiles, submits).        |
| `oathbreaker/qiskit/poc/run_ibm.py`               | Oath-4 POC runner (builds in-Python, transpiles, submits). |
| `oathbreaker/crates/benchmark/src/main.rs`        | `export-all-qasm` CLI; emits the OpenQASM 3.0 artefacts.   |
| `oathbreaker/qiskit/compilers.py`                 | Pluggable Qiskit / TKET compiler abstraction.              |
| `oathbreaker/qiskit/backends.py`                  | `BackendSpec` catalogue; live calibration data is layered on top at run time. |

For the algorithmic background, start with
[CIRCUIT_ARCHITECTURE](/docs/circuit-architecture) and
[NISQ_ROADMAP](/docs/nisq-roadmap). For why running an Oath-8+ circuit
on today's hardware is not yet meaningful, see
[LIMITATIONS](/docs/limitations).
