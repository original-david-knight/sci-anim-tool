# Choose the GPU

`frame`, `frames`, `render` and `check-layout` draw on a discrete GPU when the machine has one, then on an integrated one, then on any other. `--gpu KIND` draws on the first GPU of that GPU kind instead: `discrete`, `integrated`, `other`, `virtual` or `cpu`. The report of `frame`, `frames` and `render` names the GPU in `gpu`, with `name`, `kind`, `backend` and `driver`. A kind the machine has no GPU of is a `gpu` error, exit 2, whose message lists every GPU the machine has.

## Sub-features

- `gpu-default` draws on the discrete GPU of a machine with a discrete and an integrated one, and names it in `.gpu`.
- `gpu-busy` keeps the discrete GPU busy during a render, as `nvidia-smi` shows on an NVIDIA machine.
- `gpu-choice` draws on the integrated GPU with `--gpu integrated`, in `frame`, `frames`, `render` and `check-layout`.
- `gpu-missing` is a `gpu` error for a kind the machine has none of, listing the GPUs it has, and writes nothing.
- `gpu-usage` is a `usage` error for a word that is no GPU kind.
- `gpu-one` draws on a machine with one GPU, such as CI's lavapipe, as before, and names it.

## How to get to it (user POV)

- Run `sci_anim frame`, `frames`, `render` or `check-layout` with or without `--gpu KIND`, and read `.gpu` in the report of the first three.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.
- For `gpu-default`, `gpu-busy` and `gpu-choice`, the machine has a discrete and an integrated GPU, such as derp1 (Intel Iris Xe and RTX 4000 Ada) or cosmic (RTX 4090 and Radeon 610M). On another machine, report those as not verified, with the GPUs `gpu-missing` lists.

- **Default.** Run `$SKILL/scripts/run "$RUN" gpu-default-frame frame examples/timeline.json -t 1 -o "$RUN/default.png"`. Exit 0. `.gpu.kind` is `discrete` and `.gpu.name` names the discrete GPU, such as `NVIDIA RTX 4000 Ada Generation Laptop GPU`.
- **Busy.** Make a 4K scene: `jq '.scene_config = {width: 3840, height: 2160, fps: 60}' skills/sci-anim/examples/derivative.json > "$RUN/derivative-4k.json"`. Start `nvidia-smi --query-gpu=timestamp,name,utilization.gpu --format=csv -lms 1000 > "$RUN/gpu-busy.nvidia-smi.csv" &` and keep its PID, run `$SKILL/scripts/run "$RUN" gpu-busy-render render "$RUN/derivative-4k.json" -o "$RUN/derivative-4k.mp4" --updates`, then kill that PID. Exit 0, `.gpu.kind` `discrete`, and most samples in the CSV are well above 0 %. Then render again with `--gpu integrated` as `gpu-choice-render-4k` under a second sampler: the report names the integrated GPU, the samples stay at 0 %, and the render takes longer.
- **Choice in each command.** Run `frame ... --gpu integrated` as `gpu-choice-frame`, `frames examples/timeline.json -o "$RUN/integrated-frames" --gpu integrated` as `gpu-choice-frames`, and `check-layout examples/timeline.json --out "$RUN/check" --gpu integrated` as `gpu-choice-check-layout`. The first two have `.gpu.kind` `integrated`; check-layout exits 0 or 1 and writes `contact-sheet-1.png`. View `default.png` and the integrated frame: both show the same frame.
- **Missing kind.** Run `frame examples/timeline.json -t 1 -o "$RUN/work/none.png" --gpu virtual` as `gpu-missing`. Exit 2, `.errors[0].kind` `gpu`, the message starts `--gpu virtual names no GPU of this machine, which has` and lists each GPU with its kind and backend, and `$RUN/work/none.png` does not exist.
- **Not a kind.** Run with `--gpu nvidia` as `gpu-usage`. Exit 2, `.errors[0].kind` `usage`, and the message lists the possible values.
- **One GPU.** On a machine with one GPU, `cargo test --release --test gpu` passes, and a `frame` with no `--gpu` names that GPU. CI runs it on lavapipe, whose `.gpu.kind` is `cpu`.
- **Proof.** The reports, the nvidia-smi samples, the two frames and the contact sheet in `$RUN`.

## Gotchas

- wgpu lists each NVIDIA GPU twice, once through Vulkan and once through OpenGL as kind `other`. `--gpu other` picks the OpenGL one, which can fail with `the GPU device failed: Parent device is lost`, as it does on cosmic. That is a `gpu` error from the driver, not a missing kind.
- `nvidia-smi` sees only NVIDIA GPUs. The integrated GPU's work does not show in it, so a sample at 0 % during a `--gpu integrated` render is the expected result, and the report's `.gpu` is the proof of which GPU drew.
- `WGPU_POWER_PREF=low-power` still changes the default, and `WGPU_BACKEND` limits which drivers are listed. Unset both before you drive this feature.
