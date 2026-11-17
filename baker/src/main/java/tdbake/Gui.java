package tdbake;

import javax.imageio.ImageIO;
import javax.swing.*;
import javax.swing.border.EmptyBorder;
import java.awt.*;
import java.awt.image.BufferedImage;
import java.io.BufferedReader;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import java.util.Properties;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

/**
 * The baker's window: everything the command line does, with the settings remembered in
 * {@code tdbake-gui.properties} beside the jar. Each job runs as a child {@code java -jar tdbake.jar ...}
 * so Stop really frees the GPU and a crash never takes the window down.
 */
public final class Gui {
    private static final Pattern PROGRESS = Pattern.compile("^\\[(\\d+)/(\\d+)] tile .*?([0-9.]+) s/tile\\s+~(.+?) left");
    private static final Pattern EXPLORER = Pattern.compile("Explorer: (http://\\S+)");

    private final Path home;
    private final Path settingsFile;
    private final Properties settings = new Properties();

    private final JFrame frame = new JFrame("Terrain Diffusion для Horizon");
    private final JTextField name = new JTextField(14);
    private final JTextField seed = new JTextField(14);
    private final JComboBox<String> scale = new JComboBox<>(new String[]{
            "1 (30 м/блок, гори до ~330)", "2 (15 м/блок, гори до ~670)", "3 (10 м/блок, гори до ~1000)",
            "4 (7,5 м/блок, гори до ~1330)", "5 (6 м/блок, гори до ~1670)", "6 (5 м/блок, гори до ~2000)"});
    private final JTextField centerX = new JTextField("0", 6);
    private final JTextField centerZ = new JTextField("0", 6);
    private final JComboBox<String> size = new JComboBox<>(new String[]{"2048", "4096", "8192", "10240", "16384", "20480", "40960"});
    private final JTextField models = new JTextField(34);
    private final JTextField server = new JTextField(34);
    private final JLabel estimate = new JLabel(" ");
    private final JProgressBar progress = new JProgressBar(0, 1000);
    private final JTextArea log = new JTextArea(14, 90);
    private final JButton exploreBtn = new JButton("Explorer: вибрати сід і місце");
    private final JButton bakeBtn = new JButton("Рахувати карту");
    private final JButton stopBtn = new JButton("Стоп");
    private final JButton previewBtn = new JButton("Переглянути карту");
    private final JButton folderBtn = new JButton("Відкрити папку карти");
    private final JButton serverBtn = new JButton("Підключити карту до сервера");

    private Process job;
    private String jobName = "";

    private Gui(Path home) {
        this.home = home;
        this.settingsFile = home.resolve("tdbake-gui.properties");
    }

    /** Opens the window. {@code home} is the jar's folder. */
    public static void open(Path home) {
        SwingUtilities.invokeLater(() -> {
            try {
                UIManager.setLookAndFeel(UIManager.getSystemLookAndFeelClassName());
            } catch (Exception ignored) {
            }
            new Gui(home).build();
        });
    }

    /** {@code dist/} -> the tools folder above it; maps live in {@code <tools>/maps}. */
    private Path mapsRoot() {
        Path parent = home.getFileName() != null && home.getFileName().toString().equalsIgnoreCase("dist") && home.getParent() != null
                ? home.getParent() : home;
        return parent.resolve("maps");
    }

    private Path mapDir() {
        String n = name.getText().trim().replaceAll("[\\\\/:*?\"<>|]", "_");
        return mapsRoot().resolve(n.isEmpty() ? "map" : n);
    }

    private int scaleValue() {
        return scale.getSelectedIndex() + 1;
    }

    private void build() {
        load();
        frame.setDefaultCloseOperation(WindowConstants.DO_NOTHING_ON_CLOSE);
        frame.addWindowListener(new java.awt.event.WindowAdapter() {
            @Override
            public void windowClosing(java.awt.event.WindowEvent e) {
                if (job != null && job.isAlive()) {
                    int r = JOptionPane.showConfirmDialog(frame, "Зупинити \"" + jobName + "\" і закрити?\n"
                            + "Готові квадрати карти збережені, наступного разу рахунок продовжиться.",
                            "Закрити", JOptionPane.OK_CANCEL_OPTION);
                    if (r != JOptionPane.OK_OPTION) return;
                    job.descendants().forEach(ProcessHandle::destroy);
                    job.destroy();
                }
                save();
                System.exit(0);
            }
        });

        JPanel form = new JPanel(new GridBagLayout());
        form.setBorder(new EmptyBorder(10, 10, 4, 10));
        GridBagConstraints c = new GridBagConstraints();
        c.insets = new Insets(3, 4, 3, 4);
        c.anchor = GridBagConstraints.WEST;
        int row = 0;
        row = line(form, c, row, "Назва карти", name, new JLabel("папка maps\\<назва> поруч з інструментом"));
        JButton random = new JButton("Випадковий");
        random.addActionListener(e -> seed.setText(Long.toString(new java.util.Random().nextLong())));
        row = line(form, c, row, "Сід", seed, random);
        row = line(form, c, row, "Масштаб", scale, new JLabel("1 рекомендую: висота світу майже ванільна"));
        JPanel center = new JPanel(new FlowLayout(FlowLayout.LEFT, 4, 0));
        center.add(new JLabel("X"));
        center.add(centerX);
        center.add(new JLabel("Z"));
        center.add(centerZ);
        row = line(form, c, row, "Центр карти", center, new JLabel("блоки; з Explorer"));
        size.setEditable(true);
        row = line(form, c, row, "Розмір (блоків)", size, estimate);
        JButton pickModels = new JButton("...");
        pickModels.addActionListener(e -> pickFolder(models, "Папка з моделями Terrain Diffusion"));
        row = line(form, c, row, "Моделі", models, pickModels);
        JButton pickServer = new JButton("...");
        pickServer.addActionListener(e -> pickFolder(server, "Папка сервера Horizon"));
        row = line(form, c, row, "Сервер Horizon", server, pickServer);

        JPanel buttons = new JPanel(new FlowLayout(FlowLayout.LEFT, 6, 4));
        for (JButton b : List.of(exploreBtn, bakeBtn, stopBtn, previewBtn, folderBtn, serverBtn)) buttons.add(b);
        stopBtn.setEnabled(false);
        exploreBtn.addActionListener(e -> explore());
        bakeBtn.addActionListener(e -> bake());
        stopBtn.addActionListener(e -> stop());
        previewBtn.addActionListener(e -> preview());
        folderBtn.addActionListener(e -> openFolder());
        serverBtn.addActionListener(e -> connectServer());

        progress.setStringPainted(true);
        progress.setString("");
        log.setEditable(false);
        log.setFont(new Font(Font.MONOSPACED, Font.PLAIN, 12));
        JPanel south = new JPanel(new BorderLayout(4, 4));
        south.setBorder(new EmptyBorder(0, 10, 10, 10));
        south.add(progress, BorderLayout.NORTH);
        south.add(new JScrollPane(log), BorderLayout.CENTER);

        JPanel top = new JPanel(new BorderLayout());
        top.add(form, BorderLayout.CENTER);
        top.add(buttons, BorderLayout.SOUTH);
        frame.getContentPane().add(top, BorderLayout.NORTH);
        frame.getContentPane().add(south, BorderLayout.CENTER);

        javax.swing.event.DocumentListener upd = new javax.swing.event.DocumentListener() {
            public void insertUpdate(javax.swing.event.DocumentEvent e) { updateEstimate(); }
            public void removeUpdate(javax.swing.event.DocumentEvent e) { updateEstimate(); }
            public void changedUpdate(javax.swing.event.DocumentEvent e) { updateEstimate(); }
        };
        ((JTextField) size.getEditor().getEditorComponent()).getDocument().addDocumentListener(upd);
        size.addActionListener(e -> updateEstimate());
        updateEstimate();

        frame.pack();
        frame.setLocationRelativeTo(null);
        frame.setVisible(true);
        say("Карти зберігаються в " + mapsRoot());
    }

    private int line(JPanel p, GridBagConstraints c, int row, String label, JComponent field, JComponent extra) {
        c.gridy = row;
        c.gridx = 0;
        c.fill = GridBagConstraints.NONE;
        c.weightx = 0;
        p.add(new JLabel(label), c);
        c.gridx = 1;
        c.fill = GridBagConstraints.HORIZONTAL;
        c.weightx = 1;
        p.add(field, c);
        c.gridx = 2;
        c.fill = GridBagConstraints.NONE;
        c.weightx = 0;
        p.add(extra, c);
        return row + 1;
    }

    private int sizeValue() {
        try {
            Object v = size.getEditor().getItem();
            return Math.max(256, Integer.parseInt(String.valueOf(v).trim()));
        } catch (NumberFormatException e) {
            return 0;
        }
    }

    private void updateEstimate() {
        int s = sizeValue();
        if (s <= 0) {
            estimate.setText("число блоків, напр. 10240");
            return;
        }
        long tiles = (long) Math.ceil(s / 256.0) * (long) Math.ceil(s / 256.0);
        double minutes = tiles * 0.5 / 60.0;
        double mb = tiles * 0.19;
        estimate.setText(String.format("%,d квадратів, ~%s при 0,5 с/квадрат, ~%s", tiles,
                minutes >= 90 ? String.format("%.1f год", minutes / 60) : String.format("%.0f хв", Math.max(1, minutes)),
                mb >= 1024 ? String.format("%.1f ГБ", mb / 1024) : String.format("%.0f МБ", mb)));
    }

    private void pickFolder(JTextField field, String title) {
        JFileChooser ch = new JFileChooser(field.getText().isBlank() ? null : new File(field.getText()));
        ch.setDialogTitle(title);
        ch.setFileSelectionMode(JFileChooser.DIRECTORIES_ONLY);
        if (ch.showOpenDialog(frame) == JFileChooser.APPROVE_OPTION) field.setText(ch.getSelectedFile().getAbsolutePath());
    }

    // ---------------------------------------------------------------------------------------------

    private boolean checkSeed() {
        String text = seed.getText().trim();
        try {
            Long.parseLong(text);
            return true;
        } catch (NumberFormatException e) {
            // Explorer shows seeds unsigned (up to 18446744073709551615): the same 64 bits.
            try {
                seed.setText(Long.toString(Long.parseUnsignedLong(text)));
                return true;
            } catch (NumberFormatException ignored) {
            }
            JOptionPane.showMessageDialog(frame, "Сід має бути цілим числом (можна з мінусом).");
            return false;
        }
    }

    private void explore() {
        if (!checkSeed()) return;
        save();
        run("Explorer", List.of("explore", "--seed", seed.getText().trim(), "--scale", Integer.toString(scaleValue())), null);
    }

    private void bake() {
        if (!checkSeed()) return;
        int s = sizeValue();
        if (s <= 0) {
            JOptionPane.showMessageDialog(frame, "Розмір має бути числом блоків, напр. 10240.");
            return;
        }
        int cx, cz;
        try {
            cx = Integer.parseInt(centerX.getText().trim());
            cz = Integer.parseInt(centerZ.getText().trim());
        } catch (NumberFormatException e) {
            JOptionPane.showMessageDialog(frame, "Центр: X і Z мають бути цілими числами.");
            return;
        }
        Path dir = mapDir();
        try {
            Bake.MapMeta m = Bake.MapMeta.read(dir.resolve("map.toml"));
            if (m != null && (m.seed != Long.parseLong(seed.getText().trim()) || m.scale != scaleValue())) {
                JOptionPane.showMessageDialog(frame, "Карта \"" + name.getText().trim() + "\" вже порахована з сідом " + m.seed
                        + " і масштабом " + m.scale + ".\nДай нову назву, щоб рахувати інший сід або масштаб.");
                return;
            }
        } catch (IOException | RuntimeException ignored) {
        }
        save();
        run("Рахую карту", List.of("bake", "--seed", seed.getText().trim(), "--scale", Integer.toString(scaleValue()),
                "--center", Integer.toString(cx), Integer.toString(cz), "--size", Integer.toString(s),
                "--out", dir.toString()), () -> {
            say("Малюю перегляд...");
            run("Перегляд", List.of("preview", "--out", dir.toString()), this::showPreview);
        });
    }

    private void preview() {
        Path dir = mapDir();
        if (!Files.isRegularFile(dir.resolve("map.toml"))) {
            JOptionPane.showMessageDialog(frame, "Карти \"" + name.getText().trim() + "\" ще немає в " + dir);
            return;
        }
        run("Перегляд", List.of("preview", "--out", dir.toString()), this::showPreview);
    }

    private void showPreview() {
        Path png = mapDir().resolve("preview.png");
        try {
            BufferedImage img = ImageIO.read(png.toFile());
            if (img == null) return;
            int max = 900;
            double k = Math.min(1.0, (double) max / Math.max(img.getWidth(), img.getHeight()));
            Image shown = k < 1 ? img.getScaledInstance((int) (img.getWidth() * k), (int) (img.getHeight() * k), Image.SCALE_SMOOTH) : img;
            JDialog d = new JDialog(frame, "Карта " + name.getText().trim() + " (північ угорі)", false);
            d.getContentPane().add(new JScrollPane(new JLabel(new ImageIcon(shown))));
            d.pack();
            d.setLocationRelativeTo(frame);
            d.setVisible(true);
        } catch (IOException e) {
            say("Не вдалося відкрити " + png + ": " + e.getMessage());
        }
    }

    private void openFolder() {
        Path dir = mapDir();
        try {
            Files.createDirectories(dir);
            Desktop.getDesktop().open(dir.toFile());
        } catch (Exception e) {
            say("Папка: " + dir);
        }
    }

    /** Points the server's config/tdmap.toml at this map and puts the height data pack into its world. */
    private void connectServer() {
        Path dir = mapDir();
        Path srv = Path.of(server.getText().trim());
        if (!Files.isRegularFile(dir.resolve("map.toml"))) {
            JOptionPane.showMessageDialog(frame, "Спершу порахуй карту \"" + name.getText().trim() + "\".");
            return;
        }
        if (!Files.isDirectory(srv)) {
            JOptionPane.showMessageDialog(frame, "Немає папки сервера " + srv);
            return;
        }
        try {
            String level = "world";
            Path props = srv.resolve("server.properties");
            if (Files.isRegularFile(props)) {
                Properties p = new Properties();
                try (InputStream in = Files.newInputStream(props)) {
                    p.load(in);
                }
                level = p.getProperty("level-name", "world").trim();
            }
            Path world = srv.resolve(level);
            boolean worldExists = Files.isDirectory(world.resolve("region")) || Files.isRegularFile(world.resolve("level.dat"));
            String msg = "Сервер: " + srv + "\nСвіт: " + world + "\nКарта: " + dir + "\n\n"
                    + "1. config\\tdmap.toml: map = '" + dir + "'\n"
                    + "2. датапак tdmap-height -> " + world.resolve("datapacks") + "\n\n"
                    + (worldExists
                    ? "УВАГА: світ уже існує. Карта діє лише на нові чанки, а висота світу\n"
                    + "з датапака застосовується лише до нового світу. Перейменуй стару папку світу перед запуском.\n\n"
                    : "")
                    + "Сервер має бути вимкнений. Продовжити?";
            if (JOptionPane.showConfirmDialog(frame, msg, "Підключити карту", JOptionPane.OK_CANCEL_OPTION) != JOptionPane.OK_OPTION) return;

            Path cfg = srv.resolve("config").resolve("tdmap.toml");
            Files.createDirectories(cfg.getParent());
            String line = "map = '" + dir + "'";
            if (Files.isRegularFile(cfg)) {
                List<String> out = new ArrayList<>();
                boolean done = false;
                for (String l : Files.readAllLines(cfg, StandardCharsets.UTF_8)) {
                    if (!done && l.trim().matches("map\\s*=.*")) {
                        out.add(line);
                        done = true;
                    } else {
                        out.add(l);
                    }
                }
                if (!done) out.add(0, line);
                Files.write(cfg, out, StandardCharsets.UTF_8);
            } else {
                Files.writeString(cfg, "# tdmap: the full list of settings is written by the mod on its first start.\n" + line + "\n");
            }
            Path pack = dir.resolve("datapack").resolve("tdmap-height");
            Path target = world.resolve("datapacks").resolve("tdmap-height");
            copyTree(pack, target);
            say("Підключено: " + cfg + " -> " + dir + "; датапак у " + target);
            JOptionPane.showMessageDialog(frame, "Готово. Запусти сервер: у лозі має бути рядок\n[horizon::tdmap] map " + dir + ": ...");
        } catch (IOException e) {
            JOptionPane.showMessageDialog(frame, "Не вдалося: " + e.getMessage());
        }
    }

    private static void copyTree(Path from, Path to) throws IOException {
        try (Stream<Path> s = Files.walk(from)) {
            for (Path p : (Iterable<Path>) s::iterator) {
                Path t = to.resolve(from.relativize(p).toString());
                if (Files.isDirectory(p)) Files.createDirectories(t);
                else Files.copy(p, t, StandardCopyOption.REPLACE_EXISTING);
            }
        }
    }

    // ---------------------------------------------------------------------------------------------

    private void run(String title, List<String> args, Runnable after) {
        if (job != null && job.isAlive()) {
            JOptionPane.showMessageDialog(frame, "Зараз працює \"" + jobName + "\". Зупини його кнопкою Стоп.");
            return;
        }
        List<String> cmd = new ArrayList<>();
        cmd.add(javaExe());
        cmd.add("-Dtd.inference.offload_models=false");
        cmd.add("-Dtdbake.home=" + home);
        if (!models.getText().isBlank()) cmd.add("-Dtdbake.models=" + models.getText().trim());
        cmd.add("-jar");
        cmd.add(jarPath().toString());
        cmd.addAll(args);
        say("> " + title);
        try {
            ProcessBuilder pb = new ProcessBuilder(cmd).directory(home.toFile()).redirectErrorStream(true);
            job = pb.start();
        } catch (IOException e) {
            say("Не вдалося запустити Java: " + e.getMessage());
            return;
        }
        jobName = title;
        setBusy(true, title);
        Process p = job;
        Thread t = new Thread(() -> {
            try (BufferedReader r = new BufferedReader(new InputStreamReader(p.getInputStream(), StandardCharsets.UTF_8))) {
                String l;
                while ((l = r.readLine()) != null) {
                    String line = l;
                    if (line.contains("LocalTerrainProvider") && (line.contains("uncached region") || line.contains("finished generating"))) {
                        continue;
                    }
                    Matcher m = PROGRESS.matcher(line);
                    Matcher ex = EXPLORER.matcher(line);
                    SwingUtilities.invokeLater(() -> {
                        if (m.find()) {
                            int done = Integer.parseInt(m.group(1)), total = Integer.parseInt(m.group(2));
                            progress.setValue((int) (1000L * done / total));
                            progress.setString(done + " / " + total + " квадратів, " + m.group(3) + " с/квадрат, лишилось ~" + m.group(4));
                        }
                        say(line);
                    });
                    if (ex.find()) {
                        try {
                            Desktop.getDesktop().browse(URI.create(ex.group(1)));
                        } catch (Exception ignored) {
                        }
                    }
                }
            } catch (IOException ignored) {
            }
            int code;
            try {
                code = p.waitFor();
            } catch (InterruptedException e) {
                code = -1;
            }
            int exit = code;
            SwingUtilities.invokeLater(() -> {
                setBusy(false, "");
                if (exit == 0) {
                    say("Готово: " + title);
                    if (after != null) after.run();
                } else if (stopped) {
                    say("Зупинено: " + title);
                } else {
                    say("ПОМИЛКА (" + exit + "): " + title + ". Подробиці вище.");
                    JOptionPane.showMessageDialog(frame, title + ": помилка. Подробиці в журналі внизу вікна.");
                }
                stopped = false;
            });
        }, "tdbake-job");
        t.setDaemon(true);
        t.start();
    }

    private boolean stopped;

    private void stop() {
        if (job != null && job.isAlive()) {
            stopped = true;
            job.descendants().forEach(ProcessHandle::destroy);
            job.destroy();
        }
    }

    private void setBusy(boolean busy, String title) {
        for (JButton b : List.of(exploreBtn, bakeBtn, previewBtn, serverBtn)) b.setEnabled(!busy);
        stopBtn.setEnabled(busy);
        if (busy) {
            progress.setIndeterminate(!title.startsWith("Рахую"));
            progress.setValue(0);
            progress.setString(title + "...");
        } else {
            progress.setIndeterminate(false);
        }
    }

    private void say(String s) {
        log.append(s + "\n");
        if (log.getLineCount() > 3000) {
            try {
                log.replaceRange("", 0, log.getLineEndOffset(500));
            } catch (Exception ignored) {
            }
        }
        log.setCaretPosition(log.getDocument().getLength());
    }

    private static String javaExe() {
        Path bin = Path.of(System.getProperty("java.home"), "bin");
        Path java = bin.resolve(isWindows() ? "java.exe" : "java");
        return Files.isRegularFile(java) ? java.toString() : "java";
    }

    private static boolean isWindows() {
        return System.getProperty("os.name", "").toLowerCase().contains("win");
    }

    static Path jarPath() {
        try {
            return Path.of(Gui.class.getProtectionDomain().getCodeSource().getLocation().toURI()).toAbsolutePath();
        } catch (Exception e) {
            return Path.of("tdbake.jar").toAbsolutePath();
        }
    }

    // ---------------------------------------------------------------------------------------------

    private void load() {
        try (InputStream in = Files.newInputStream(settingsFile)) {
            settings.load(new InputStreamReader(in, StandardCharsets.UTF_8));
        } catch (IOException ignored) {
        }
        name.setText(settings.getProperty("name", "map"));
        seed.setText(settings.getProperty("seed", "12345"));
        scale.setSelectedIndex(Math.max(0, Math.min(5, Integer.parseInt(settings.getProperty("scale", "1")) - 1)));
        centerX.setText(settings.getProperty("centerX", "0"));
        centerZ.setText(settings.getProperty("centerZ", "0"));
        size.setSelectedItem(settings.getProperty("size", "10240"));
        String appdata = System.getenv("APPDATA");
        models.setText(settings.getProperty("models",
                appdata != null ? Path.of(appdata, ".minecraft", "terrain-diffusion-models").toString() : ""));
        server.setText(settings.getProperty("server", "F:\\MinecraftServers\\horizonTestServer"));
    }

    private void save() {
        settings.setProperty("name", name.getText().trim());
        settings.setProperty("seed", seed.getText().trim());
        settings.setProperty("scale", Integer.toString(scaleValue()));
        settings.setProperty("centerX", centerX.getText().trim());
        settings.setProperty("centerZ", centerZ.getText().trim());
        settings.setProperty("size", String.valueOf(size.getEditor().getItem()).trim());
        settings.setProperty("models", models.getText().trim());
        settings.setProperty("server", server.getText().trim());
        try (OutputStream out = Files.newOutputStream(settingsFile)) {
            settings.store(new java.io.OutputStreamWriter(out, StandardCharsets.UTF_8), "tdbake window settings");
        } catch (IOException ignored) {
        }
    }
}
