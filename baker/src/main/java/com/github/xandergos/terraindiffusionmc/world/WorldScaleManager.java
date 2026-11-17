package com.github.xandergos.terraindiffusionmc.world;

/**
 * Headless replacement for the mod's per-world scale: the baker sets it once from
 * the command line ({@code --scale}). Same bounds as the mod (1..6).
 */
public final class WorldScaleManager {
    public static final int DEFAULT_SCALE = 1;
    private static final int MIN_SCALE = 1;
    public static final int MAX_SCALE = 6;

    private static volatile int currentScale = DEFAULT_SCALE;

    private WorldScaleManager() {}

    public static int getCurrentScale() {
        return currentScale;
    }

    public static void setCurrentScale(int configuredScale) {
        currentScale = clampScale(configuredScale);
    }

    public static int clampScale(int configuredScale) {
        return Math.max(MIN_SCALE, Math.min(MAX_SCALE, configuredScale));
    }
}
