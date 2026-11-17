package com.github.xandergos.terraindiffusionmc.pipeline;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;

/**
 * Headless replacement for the mod's asset manager. Models come from
 * {@code -Dtdbake.models} (default {@code <home>/terrain-diffusion-models}); point it at the
 * folder the Fabric mod already downloaded ({@code .minecraft/terrain-diffusion-models}) to
 * skip the 2.5 GB download. Missing files are fetched from the same pinned Hugging Face
 * commit the mod 2.2.0 uses.
 */
public final class ModelAssetManager {
    private static final Logger LOG = LoggerFactory.getLogger(ModelAssetManager.class);
    private static final String REPO = "xandergos/terrain-diffusion-30m-onnx";
    private static final String REVISION = "ad2df557eca5645f588766101cf3bc3682455c3e";
    private static final List<String> FILES = List.of(
            "base_model.onnx", "coarse_model.onnx", "decoder_model.onnx",
            "pipeline_data.json", "world_pipeline_config.json");
    private static volatile boolean ready = false;

    private ModelAssetManager() {}

    public static Path modelDirectory() {
        String explicit = System.getProperty("tdbake.models");
        if (explicit != null && !explicit.isBlank()) return Path.of(explicit).toAbsolutePath().normalize();
        return Path.of(System.getProperty("tdbake.home", ".")).toAbsolutePath().normalize()
                .resolve("terrain-diffusion-models");
    }

    public static synchronized void ensureAssetsReady() {
        if (ready) return;
        Path dir = modelDirectory();
        try {
            Files.createDirectories(dir);
            for (String name : FILES) {
                Path file = dir.resolve(name);
                if (Files.isRegularFile(file) && Files.size(file) > 0) continue;
                download(name, file);
            }
        } catch (IOException | InterruptedException e) {
            throw new IllegalStateException("Model files are not ready in " + dir + ": " + e.getMessage(), e);
        }
        LOG.info("Models: {}", dir);
        ready = true;
    }

    public static Path resolveAssetPath(String fileName) {
        return modelDirectory().resolve(fileName);
    }

    private static void download(String name, Path target) throws IOException, InterruptedException {
        String url = "https://huggingface.co/" + REPO + "/resolve/" + REVISION + "/" + name + "?download=true";
        LOG.info("Downloading {} ...", name);
        HttpClient client = HttpClient.newBuilder().followRedirects(HttpClient.Redirect.ALWAYS).build();
        HttpResponse<InputStream> response = client.send(HttpRequest.newBuilder(URI.create(url)).build(),
                HttpResponse.BodyHandlers.ofInputStream());
        if (response.statusCode() != 200) {
            throw new IOException("HTTP " + response.statusCode() + " for " + url);
        }
        Path part = target.resolveSibling(name + ".part");
        try (InputStream in = response.body()) {
            Files.copy(in, part, StandardCopyOption.REPLACE_EXISTING);
        }
        Files.move(part, target, StandardCopyOption.REPLACE_EXISTING);
        LOG.info("Downloaded {} ({} MB)", name, Files.size(target) / (1024 * 1024));
    }
}
