package tdbake;

import com.github.xandergos.terraindiffusionmc.explorer.ExplorerServer;
import com.github.xandergos.terraindiffusionmc.pipeline.LocalTerrainProvider;
import com.github.xandergos.terraindiffusionmc.pipeline.LocalTerrainProvider.HeightmapData;
import com.github.xandergos.terraindiffusionmc.pipeline.ModelAssetManager;
import com.github.xandergos.terraindiffusionmc.pipeline.OnnxModel;
import com.github.xandergos.terraindiffusionmc.pipeline.PipelineModels;
import com.github.xandergos.terraindiffusionmc.world.HeightConverter;
import com.github.xandergos.terraindiffusionmc.world.WorldScaleManager;

import javax.imageio.ImageIO;
import java.awt.image.BufferedImage;
import java.io.IOException;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Properties;

/**
 * Terrain Diffusion map baker for the Horizon {@code tdmap} mod.
 *
 * <p>Runs the Terrain Diffusion pipeline of the Fabric mod (same models, same code) without
 * Minecraft and writes a finite map: 256x256-block tiles of surface height (Minecraft Y) and
 * biome. The server mod only reads these files, so the server needs no GPU.
 *
 * <pre>
 * java -jar tdbake.jar explore --seed 123
 * java -jar tdbake.jar bake --seed 123 --scale 1 --center 0 0 --size 8192 --out F:\tdmap
 * java -jar tdbake.jar preview --out F:\tdmap
 * </pre>
 */
public final class Bake {
    static final int TILE = 256;
    static final byte[] MAGIC = "TDMAP1\0\0".getBytes(StandardCharsets.US_ASCII);
    static final int TILE_BYTES = MAGIC.length + TILE * TILE * 2 + TILE * TILE;

    private Bake() {}

    public static void main(String[] args) throws Exception {
        if (args.length == 0 && !java.awt.GraphicsEnvironment.isHeadless()) {
            Gui.open(Gui.jarPath().getParent());
            return;
        }
        if (args.length == 0 || args[0].equals("help") || args[0].equals("--help")) {
            usage();
            return;
        }
        Map<String, List<String>> opts = parse(args);
        switch (args[0]) {
            case "explore" -> explore(opts);
            case "bake" -> bake(opts);
            case "preview" -> preview(opts);
            default -> {
                System.err.println("Unknown command: " + args[0]);
                usage();
                System.exit(2);
            }
        }
    }

    private static void usage() {
        System.out.println("""
                tdbake: Terrain Diffusion map baker for the Horizon tdmap mod

                  explore  --seed N [--scale 1]
                      Opens the Terrain Diffusion explorer (http://127.0.0.1:19801) to pick a seed and an area.
                  bake     --seed N --scale 1 --center X Z --size BLOCKS --out DIR
                      (or --from X1 Z1 --to X2 Z2 instead of --center/--size)
                      Bakes the map into DIR. Stops safely at any time (Ctrl+C) and continues where it stopped.
                  preview  --out DIR [--step 8]
                      Writes DIR/preview.png (1 pixel = step blocks) from the tiles baked so far.

                Options for all commands (JVM properties, before -jar):
                  -Dtdbake.models=PATH   folder with the models (e.g. %APPDATA%\\.minecraft\\terrain-diffusion-models);
                                         missing files are downloaded (~2.5 GB)
                  -Dtdbake.home=PATH     folder for config/ and onnx-cache/ (current folder by default)
                """);
    }

    // ---------------------------------------------------------------------------------------------

    private static void explore(Map<String, List<String>> opts) throws Exception {
        long seed = longOpt(opts, "seed", 0L);
        WorldScaleManager.setCurrentScale(intOpt(opts, "scale", 1));
        start(seed);
        int port = ExplorerServer.startIfNotRunning();
        System.out.println("Explorer: http://127.0.0.1:" + port + "  (seed " + seed + "). Ctrl+C to stop.");
        Thread.currentThread().join();
    }

    private static void start(long seed) {
        // Download first (can take longer than the pipeline's 10 minute wait for its models).
        ModelAssetManager.ensureAssetsReady();
        PipelineModels.load();
        LocalTerrainProvider.init(seed);
    }

    // ---------------------------------------------------------------------------------------------

    private static void bake(Map<String, List<String>> opts) throws Exception {
        long seed = longOpt(opts, "seed", Long.MIN_VALUE);
        if (seed == Long.MIN_VALUE) throw new IllegalArgumentException("--seed is required");
        int scale = WorldScaleManager.clampScale(intOpt(opts, "scale", 1));
        Path out = Path.of(str(opts, "out", null)).toAbsolutePath().normalize();

        int x1, z1, x2, z2;
        if (opts.containsKey("center")) {
            List<String> c = opts.get("center");
            int cx = Integer.parseInt(c.get(0)), cz = Integer.parseInt(c.get(1));
            int size = intOpt(opts, "size", 4096);
            x1 = cx - size / 2;
            z1 = cz - size / 2;
            x2 = x1 + size;
            z2 = z1 + size;
        } else {
            List<String> f = opts.get("from"), t = opts.get("to");
            if (f == null || t == null) throw new IllegalArgumentException("give --center X Z --size N or --from X Z --to X Z");
            x1 = Math.min(Integer.parseInt(f.get(0)), Integer.parseInt(t.get(0)));
            z1 = Math.min(Integer.parseInt(f.get(1)), Integer.parseInt(t.get(1)));
            x2 = Math.max(Integer.parseInt(f.get(0)), Integer.parseInt(t.get(0)));
            z2 = Math.max(Integer.parseInt(f.get(1)), Integer.parseInt(t.get(1)));
        }
        int tx1 = Math.floorDiv(x1, TILE), tz1 = Math.floorDiv(z1, TILE);
        int tx2 = Math.floorDiv(x2 - 1, TILE) + 1, tz2 = Math.floorDiv(z2 - 1, TILE) + 1;

        Path tiles = out.resolve("tiles");
        Files.createDirectories(tiles);
        Path meta = out.resolve("map.toml");
        MapMeta existing = MapMeta.read(meta);
        if (existing != null && (existing.seed != seed || existing.scale != scale)) {
            throw new IllegalStateException(meta + " is for seed " + existing.seed + " scale " + existing.scale
                    + ": use another --out folder for seed " + seed + " scale " + scale);
        }
        MapMeta m = existing != null ? existing.union(tx1, tz1, tx2, tz2) : new MapMeta(seed, scale, tx1, tz1, tx2, tz2);
        m.write(meta);
        writeDatapack(out, scale);

        WorldScaleManager.setCurrentScale(scale);
        start(seed);

        List<int[]> order = spiral(tx1, tz1, tx2, tz2);
        int total = order.size();
        int done = 0, skipped = 0;
        long started = System.nanoTime();
        int maxY = Integer.MIN_VALUE, minY = Integer.MAX_VALUE;
        System.out.printf(Locale.ROOT, "Baking %d tiles (%d x %d blocks, x %d..%d, z %d..%d), seed %d, scale %d (%.1f m/block) into %s%n",
                total, (tx2 - tx1) * TILE, (tz2 - tz1) * TILE, tx1 * TILE, tx2 * TILE, tz1 * TILE, tz2 * TILE,
                seed, scale, 30.0 / scale, out);
        for (int[] t : order) {
            Path file = tiles.resolve(tileName(t[0], t[1]));
            if (Files.isRegularFile(file) && Files.size(file) == TILE_BYTES) {
                skipped++;
                continue;
            }
            int bx = t[0] * TILE, bz = t[1] * TILE;
            HeightmapData data = LocalTerrainProvider.getInstance().fetchHeightmap(bz, bx, bz + TILE, bx + TILE);
            ByteBuffer buf = ByteBuffer.allocate(TILE_BYTES).order(ByteOrder.LITTLE_ENDIAN);
            buf.put(MAGIC);
            for (int z = 0; z < TILE; z++) {
                for (int x = 0; x < TILE; x++) {
                    int y = HeightConverter.convertToMinecraftHeight(data.heightmap[z][x], scale);
                    y = Math.max(Short.MIN_VALUE, Math.min(Short.MAX_VALUE, y));
                    maxY = Math.max(maxY, y);
                    minY = Math.min(minY, y);
                    buf.putShort((short) y);
                }
            }
            for (int z = 0; z < TILE; z++) {
                for (int x = 0; x < TILE; x++) {
                    buf.put((byte) Math.max(0, Math.min(255, data.biomeIds[z][x])));
                }
            }
            Path part = tiles.resolve(tileName(t[0], t[1]) + ".part");
            Files.write(part, buf.array());
            Files.move(part, file, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
            done++;
            double secs = (System.nanoTime() - started) / 1e9;
            int left = total - done - skipped;
            double per = secs / done;
            System.out.printf(Locale.ROOT, "[%d/%d] tile %d,%d  Y %d..%d  %.1f s/tile  ~%s left  (%s)%n",
                    done + skipped, total, t[0], t[1], minY, maxY, per, duration(per * left),
                    OnnxModel.getResolvedInferenceProvider());
        }
        System.out.println("Done: " + (done + skipped) + " tiles in " + tiles
                + (skipped > 0 ? " (" + skipped + " were already baked)" : ""));
        System.exit(0);
    }

    /** Tiles from the centre outwards, so a partly baked map is usable around the middle. */
    static List<int[]> spiral(int tx1, int tz1, int tx2, int tz2) {
        double cx = (tx1 + tx2 - 1) / 2.0, cz = (tz1 + tz2 - 1) / 2.0;
        List<int[]> all = new ArrayList<>();
        for (int tz = tz1; tz < tz2; tz++) for (int tx = tx1; tx < tx2; tx++) all.add(new int[]{tx, tz});
        all.sort((a, b) -> {
            double da = Math.max(Math.abs(a[0] - cx), Math.abs(a[1] - cz));
            double db = Math.max(Math.abs(b[0] - cx), Math.abs(b[1] - cz));
            if (da != db) return Double.compare(da, db);
            if (a[1] != b[1]) return Integer.compare(a[1], b[1]);
            return Integer.compare(a[0], b[0]);
        });
        return all;
    }

    static String tileName(int tx, int tz) {
        return tx + "_" + tz + ".bin";
    }

    private static String duration(double seconds) {
        long s = (long) seconds;
        if (s >= 86400) return String.format(Locale.ROOT, "%dd %dh", s / 86400, (s % 86400) / 3600);
        if (s >= 3600) return String.format(Locale.ROOT, "%dh %dm", s / 3600, (s % 3600) / 60);
        return String.format(Locale.ROOT, "%dm %ds", s / 60, s % 60);
    }

    /**
     * A data pack that makes the Overworld tall enough for the map's peaks: vanilla 26.3's
     * dimension type with the top raised (Nether and End untouched).
     */
    private static void writeDatapack(Path out, int scale) throws IOException {
        int top = HeightConverter.getMaxGeneratedYForScale(scale) + 16;
        int minY = -64;
        int height = Math.max(384, ((top - minY) + 15) / 16 * 16);
        height = Math.min(height, 2032 - minY);
        Path pack = out.resolve("datapack").resolve("tdmap-height");
        Path dt = pack.resolve("data/minecraft/dimension_type");
        Files.createDirectories(dt);
        String json;
        try (InputStream in = Bake.class.getResourceAsStream("/tdbake/overworld_dimension_type.json")) {
            json = new String(in.readAllBytes(), StandardCharsets.UTF_8);
        }
        json = json.replaceFirst("\"height\"\\s*:\\s*\\d+", "\"height\": " + height)
                .replaceFirst("\"logical_height\"\\s*:\\s*\\d+", "\"logical_height\": " + height)
                .replaceFirst("\"min_y\"\\s*:\\s*-?\\d+", "\"min_y\": " + minY);
        Files.writeString(dt.resolve("overworld.json"), json);
        Files.writeString(pack.resolve("pack.mcmeta"), """
                {
                  "pack": {
                    "description": "tdmap: Overworld from y %d to %d for Terrain Diffusion scale %d",
                    "pack_format": 121,
                    "min_format": [121, 0],
                    "max_format": [121, 0]
                  }
                }
                """.formatted(minY, minY + height - 1, scale));
    }

    // ---------------------------------------------------------------------------------------------

    private static void preview(Map<String, List<String>> opts) throws Exception {
        Path out = Path.of(str(opts, "out", null)).toAbsolutePath().normalize();
        int step = Math.max(1, intOpt(opts, "step", 8));
        MapMeta m = MapMeta.read(out.resolve("map.toml"));
        if (m == null) throw new IllegalStateException("No map.toml in " + out);
        int w = (m.tx2 - m.tx1) * TILE / step, h = (m.tz2 - m.tz1) * TILE / step;
        BufferedImage img = new BufferedImage(w, h, BufferedImage.TYPE_INT_RGB);
        int tilesFound = 0;
        for (int tz = m.tz1; tz < m.tz2; tz++) {
            for (int tx = m.tx1; tx < m.tx2; tx++) {
                Path file = out.resolve("tiles").resolve(tileName(tx, tz));
                if (!Files.isRegularFile(file) || Files.size(file) != TILE_BYTES) continue;
                tilesFound++;
                ByteBuffer buf = ByteBuffer.wrap(Files.readAllBytes(file)).order(ByteOrder.LITTLE_ENDIAN);
                for (int z = 0; z < TILE; z += step) {
                    for (int x = 0; x < TILE; x += step) {
                        int y = buf.getShort(MAGIC.length + (z * TILE + x) * 2);
                        int biome = buf.get(MAGIC.length + TILE * TILE * 2 + z * TILE + x) & 0xFF;
                        int px = ((tx - m.tx1) * TILE + x) / step, pz = ((tz - m.tz1) * TILE + z) / step;
                        if (px < w && pz < h) img.setRGB(px, pz, color(y, biome));
                    }
                }
            }
        }
        Path png = out.resolve("preview.png");
        ImageIO.write(img, "png", png.toFile());
        System.out.println("Wrote " + png + " (" + w + "x" + h + ", " + tilesFound + " tiles)");
    }

    private static int color(int y, int biome) {
        if (y < 63) {
            int d = Math.max(0, Math.min(60, 63 - y));
            return rgb(20, 60 + (60 - d), 140 + (60 - d) / 2);
        }
        int base = switch (biome) {
            case 5 -> 0xD8C98A;               // desert
            case 26 -> 0xB5653A;              // badlands
            case 3, 32, 33 -> 0xF0F4F8;       // snow
            case 16, 116 -> 0xC9D8D0;         // snowy taiga
            case 35, 19 -> 0x8C8C8C;          // stone
            case 23 -> 0x2E8B2E;              // jungle
            case 6 -> 0x4C6B3A;               // swamp
            case 8, 108 -> 0x3F7F3A;          // forest
            case 15, 115 -> 0x4A6B4E;         // taiga
            case 17 -> 0xA5A55A;              // savanna
            case 31, 29 -> 0x8FB07A;          // grove, meadow
            default -> 0x7DAA55;              // plains
        };
        double shade = 0.75 + Math.min(0.35, (y - 63) / 600.0);
        return rgb((int) (((base >> 16) & 255) * shade), (int) (((base >> 8) & 255) * shade), (int) ((base & 255) * shade));
    }

    private static int rgb(int r, int g, int b) {
        return (Math.max(0, Math.min(255, r)) << 16) | (Math.max(0, Math.min(255, g)) << 8) | Math.max(0, Math.min(255, b));
    }

    // ---------------------------------------------------------------------------------------------

    /** {@code map.toml}: what the mod needs to read the tiles. */
    static final class MapMeta {
        final long seed;
        final int scale, tx1, tz1, tx2, tz2;

        MapMeta(long seed, int scale, int tx1, int tz1, int tx2, int tz2) {
            this.seed = seed;
            this.scale = scale;
            this.tx1 = tx1;
            this.tz1 = tz1;
            this.tx2 = tx2;
            this.tz2 = tz2;
        }

        MapMeta union(int ax1, int az1, int ax2, int az2) {
            return new MapMeta(seed, scale, Math.min(tx1, ax1), Math.min(tz1, az1), Math.max(tx2, ax2), Math.max(tz2, az2));
        }

        static MapMeta read(Path file) throws IOException {
            if (!Files.isRegularFile(file)) return null;
            Properties p = new Properties();
            for (String line : Files.readAllLines(file)) {
                int eq = line.indexOf('=');
                if (eq < 0 || line.trim().startsWith("#") || line.trim().startsWith("[")) continue;
                p.setProperty(line.substring(0, eq).trim(), line.substring(eq + 1).trim().replace("\"", ""));
            }
            return new MapMeta(Long.parseLong(p.getProperty("seed")), Integer.parseInt(p.getProperty("scale")),
                    Integer.parseInt(p.getProperty("tile_x_min")), Integer.parseInt(p.getProperty("tile_z_min")),
                    Integer.parseInt(p.getProperty("tile_x_max")), Integer.parseInt(p.getProperty("tile_z_max")));
        }

        void write(Path file) throws IOException {
            Files.writeString(file, """
                    # Terrain Diffusion map baked by tdbake for the Horizon tdmap mod. Do not edit.
                    format = 1
                    seed = %d
                    scale = %d
                    meters_per_block = %s
                    tile_size = %d
                    sea_level = 63
                    # Tiles tile_x_min..tile_x_max-1, tile_z_min..tile_z_max-1: blocks x %d..%d, z %d..%d
                    tile_x_min = %d
                    tile_z_min = %d
                    tile_x_max = %d
                    tile_z_max = %d
                    """.formatted(seed, scale, Float.toString(30f / scale), TILE,
                    tx1 * TILE, tx2 * TILE - 1, tz1 * TILE, tz2 * TILE - 1, tx1, tz1, tx2, tz2));
        }
    }

    // ---------------------------------------------------------------------------------------------

    private static Map<String, List<String>> parse(String[] args) {
        Map<String, List<String>> out = new HashMap<>();
        String key = null;
        for (int i = 1; i < args.length; i++) {
            String a = args[i];
            if (a.startsWith("--") && !a.matches("--?\\d.*")) {
                key = a.substring(2);
                out.put(key, new ArrayList<>());
            } else if (key != null) {
                out.get(key).add(a);
            } else {
                throw new IllegalArgumentException("Unexpected argument: " + a);
            }
        }
        return out;
    }

    private static String str(Map<String, List<String>> o, String k, String def) {
        List<String> v = o.get(k);
        if (v == null || v.isEmpty()) {
            if (def == null) throw new IllegalArgumentException("--" + k + " is required");
            return def;
        }
        return v.get(0);
    }

    private static int intOpt(Map<String, List<String>> o, String k, int def) {
        return Integer.parseInt(str(o, k, Integer.toString(def)));
    }

    private static long longOpt(Map<String, List<String>> o, String k, long def) {
        String v = str(o, k, Long.toString(def));
        try {
            return Long.parseLong(v);
        } catch (NumberFormatException e) {
            return Long.parseUnsignedLong(v); // as Explorer shows seeds
        }
    }
}
