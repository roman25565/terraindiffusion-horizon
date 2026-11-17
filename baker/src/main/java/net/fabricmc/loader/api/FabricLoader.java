package net.fabricmc.loader.api;

import java.nio.file.Path;

/**
 * Stand-in for Fabric's loader in the headless baker: the "game" and "config"
 * folders are the baker's home ({@code -Dtdbake.home}, the current folder by default).
 */
public final class FabricLoader {
    private static final FabricLoader INSTANCE = new FabricLoader();

    private FabricLoader() {}

    public static FabricLoader getInstance() {
        return INSTANCE;
    }

    private static Path home() {
        return Path.of(System.getProperty("tdbake.home", ".")).toAbsolutePath().normalize();
    }

    public Path getGameDir() {
        return home();
    }

    public Path getConfigDir() {
        return home().resolve("config");
    }
}
