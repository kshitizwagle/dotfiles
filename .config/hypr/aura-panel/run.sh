#!/usr/bin/env bash
# Launch aura-panel. The cairo renderer avoids loading the Vulkan/GL driver (≈118 MB → ≈70 MB RSS);
# one malloc arena is plenty for a panel whose worker threads only shell out.
export GSK_RENDERER=cairo MALLOC_ARENA_MAX=1
exec ~/.local/bin/aura-panel "$@"
