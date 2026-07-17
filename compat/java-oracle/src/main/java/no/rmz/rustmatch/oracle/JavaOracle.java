/*
 * Copyright 2026 Bjorn Remseth
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
package no.rmz.rustmatch.oracle;

import com.fasterxml.jackson.core.JsonFactory;
import com.fasterxml.jackson.core.JsonGenerator;
import com.fasterxml.jackson.core.JsonParser;
import com.fasterxml.jackson.core.JsonToken;
import java.io.BufferedWriter;
import java.io.IOException;
import java.io.StringWriter;
import java.net.URISyntaxException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.regex.Pattern;
import no.rmz.rmatch.Matcher;
import no.rmz.rmatch.RMatch;
import no.rmz.rmatch.RegexpParserException;

/** Runs semantic fixtures against the exact published Java rmatch reference. */
public final class JavaOracle {

  private static final int SCHEMA_VERSION = 1;
  private static final int REQUIRED_JAVA_FEATURE = 21;
  private static final Set<String> FIXTURE_TIERS =
      Set.of("ascii-literal-v1", "ascii-predicate-v1", "ascii-composition-v1");
  private static final String ORACLE_GROUP = "no.rmz";
  private static final String ORACLE_ARTIFACT = "rmatch";
  private static final String ORACLE_VERSION = "2.0.0-RC1";
  private static final String ORACLE_SHA256 =
      "05542b4778d004bd40037539567a0fff31b65b3bf234c61ef1493046c3e71a8a";
  private static final Pattern CASE_ID = Pattern.compile("[a-z0-9][a-z0-9-]*");
  private static final Set<String> RUST_EXPECTATIONS =
      Set.of("matched", "rejected-invalid", "rejected-unsupported");
  private static final JsonFactory JSON = new JsonFactory();

  private JavaOracle() {}

  /**
   * Run the oracle.
   *
   * @param args fixture JSONL, result JSONL, and manifest JSON paths
   * @throws Exception if the environment, fixture, reference, or output is invalid
   */
  public static void main(final String[] args) throws Exception {
    if (args.length != 3) {
      throw new IllegalArgumentException(
          "usage: JavaOracle <fixtures.jsonl> <results.jsonl> <manifest.json>");
    }

    requirePinnedRuntime();
    verifyReferenceArtifact();

    final Path fixturePath = Path.of(args[0]);
    final Path resultPath = Path.of(args[1]);
    final Path manifestPath = Path.of(args[2]);
    final List<FixtureCase> fixtures = readFixtures(fixturePath);
    final List<OracleResult> results = new ArrayList<>(fixtures.size());
    for (final FixtureCase fixture : fixtures) {
      results.add(runFixture(fixture));
    }

    writeResults(resultPath, results);
    writeManifest(
        manifestPath,
        fixturePath.getFileName().toString(),
        sha256(fixturePath),
        results.size(),
        sha256(resultPath));
    System.err.printf(
        "Java oracle %s processed %d fixtures with Java %s%n",
        ORACLE_VERSION, results.size(), Runtime.version());
  }

  private static void requirePinnedRuntime() {
    final int actual = Runtime.version().feature();
    if (actual != REQUIRED_JAVA_FEATURE) {
      throw new IllegalStateException(
          "the compatibility oracle requires Java "
              + REQUIRED_JAVA_FEATURE
              + ", but is running on Java "
              + actual);
    }
  }

  private static void verifyReferenceArtifact()
      throws IOException, NoSuchAlgorithmException, URISyntaxException {
    final Path location =
        Path.of(RMatch.class.getProtectionDomain().getCodeSource().getLocation().toURI());
    if (!Files.isRegularFile(location)) {
      throw new IllegalStateException("rmatch reference did not load from a JAR: " + location);
    }
    final String actual = sha256(location);
    if (!ORACLE_SHA256.equals(actual)) {
      throw new IllegalStateException(
          "unexpected rmatch JAR SHA-256: expected " + ORACLE_SHA256 + ", got " + actual);
    }
  }

  private static List<FixtureCase> readFixtures(final Path path) throws IOException {
    final List<FixtureCase> fixtures = new ArrayList<>();
    final Set<String> caseIds = new HashSet<>();
    int lineNumber = 0;
    for (final String line : Files.readAllLines(path, StandardCharsets.UTF_8)) {
      lineNumber++;
      if (line.isBlank()) {
        continue;
      }
      final FixtureCase fixture;
      try (JsonParser parser = JSON.createParser(line)) {
        fixture = parseFixture(parser);
        if (parser.nextToken() != null) {
          throw format("content after fixture object");
        }
      } catch (FixtureFormatException exception) {
        throw new FixtureFormatException(path + ":" + lineNumber + ": " + exception.getMessage());
      }
      if (!caseIds.add(fixture.caseId())) {
        throw new FixtureFormatException(path + ":" + lineNumber + ": duplicate case_id");
      }
      fixtures.add(fixture);
    }
    if (fixtures.isEmpty()) {
      throw new FixtureFormatException(path + ": fixture set is empty");
    }
    return List.copyOf(fixtures);
  }

  private static FixtureCase parseFixture(final JsonParser parser) throws IOException {
    expect(parser.nextToken(), JsonToken.START_OBJECT, "fixture object");
    Integer schemaVersion = null;
    String caseId = null;
    String compatibilityTier = null;
    String rustExpectation = null;
    List<FixturePattern> patterns = null;
    int[] input = null;

    while (parser.nextToken() != JsonToken.END_OBJECT) {
      expect(parser.currentToken(), JsonToken.FIELD_NAME, "fixture field");
      final String field = parser.currentName();
      parser.nextToken();
      switch (field) {
        case "schema_version" -> schemaVersion = parseInt(parser, 1, 1, field);
        case "case_id" -> caseId = parseString(parser, field);
        case "compatibility_tier" -> compatibilityTier = parseString(parser, field);
        case "rust_expectation" -> rustExpectation = parseString(parser, field);
        case "patterns" -> patterns = parsePatterns(parser);
        case "input_utf16" -> input = parseCodeUnits(parser, field);
        default -> throw format("unknown fixture field: " + field);
      }
    }

    if (schemaVersion == null
        || caseId == null
        || compatibilityTier == null
        || rustExpectation == null
        || patterns == null
        || input == null) {
      throw format("fixture is missing one or more required fields");
    }
    if (!CASE_ID.matcher(caseId).matches()) {
      throw format("invalid case_id: " + caseId);
    }
    if (!FIXTURE_TIERS.contains(compatibilityTier)) {
      throw format("unsupported compatibility_tier: " + compatibilityTier);
    }
    if (!RUST_EXPECTATIONS.contains(rustExpectation)) {
      throw format("unsupported rust_expectation: " + rustExpectation);
    }
    return new FixtureCase(caseId, rustExpectation, patterns, input);
  }

  private static List<FixturePattern> parsePatterns(final JsonParser parser) throws IOException {
    expect(parser.currentToken(), JsonToken.START_ARRAY, "patterns array");
    final List<FixturePattern> patterns = new ArrayList<>();
    final Set<Long> patternIds = new HashSet<>();
    while (parser.nextToken() != JsonToken.END_ARRAY) {
      expect(parser.currentToken(), JsonToken.START_OBJECT, "pattern object");
      Long patternId = null;
      int[] codeUnits = null;
      while (parser.nextToken() != JsonToken.END_OBJECT) {
        expect(parser.currentToken(), JsonToken.FIELD_NAME, "pattern field");
        final String field = parser.currentName();
        parser.nextToken();
        switch (field) {
          case "pattern_id" -> patternId = parseLong(parser, 0, 0xffff_ffffL, field);
          case "utf16" -> codeUnits = parseCodeUnits(parser, field);
          default -> throw format("unknown pattern field: " + field);
        }
      }
      if (patternId == null || codeUnits == null) {
        throw format("pattern is missing pattern_id or utf16");
      }
      if (codeUnits.length == 0) {
        throw format("pattern utf16 must not be empty");
      }
      if (!patternIds.add(patternId)) {
        throw format("duplicate pattern_id: " + patternId);
      }
      patterns.add(new FixturePattern(patternId, codeUnits));
    }
    if (patterns.isEmpty()) {
      throw format("patterns array is empty");
    }
    return List.copyOf(patterns);
  }

  private static int[] parseCodeUnits(final JsonParser parser, final String field)
      throws IOException {
    expect(parser.currentToken(), JsonToken.START_ARRAY, field + " array");
    final List<Integer> values = new ArrayList<>();
    while (parser.nextToken() != JsonToken.END_ARRAY) {
      values.add(parseInt(parser, 0, 0xffff, field));
    }
    final int[] result = new int[values.size()];
    for (int index = 0; index < values.size(); index++) {
      result[index] = values.get(index);
    }
    return result;
  }

  private static String parseString(final JsonParser parser, final String field) throws IOException {
    expect(parser.currentToken(), JsonToken.VALUE_STRING, field + " string");
    return parser.getText();
  }

  private static int parseInt(
      final JsonParser parser, final int minimum, final int maximum, final String field)
      throws IOException {
    return Math.toIntExact(parseLong(parser, minimum, maximum, field));
  }

  private static long parseLong(
      final JsonParser parser, final long minimum, final long maximum, final String field)
      throws IOException {
    if (parser.currentToken() != JsonToken.VALUE_NUMBER_INT) {
      throw format(field + " must be an integer");
    }
    final long value = parser.getLongValue();
    if (value < minimum || value > maximum) {
      throw format(field + " is outside " + minimum + ".." + maximum);
    }
    return value;
  }

  private static void expect(
      final JsonToken actual, final JsonToken expected, final String description) {
    if (actual != expected) {
      throw format("expected " + description + ", got " + actual);
    }
  }

  private static FixtureFormatException format(final String message) {
    return new FixtureFormatException(message);
  }

  private static OracleResult runFixture(final FixtureCase fixture) {
    final List<MatchEvent> events = new ArrayList<>();
    try (Matcher matcher = RMatch.newSingleMatcher()) {
      for (final FixturePattern pattern : fixture.patterns()) {
        try {
          matcher.add(
              toString(pattern.codeUnits()),
              (buffer, start, end) ->
                  events.add(new MatchEvent(pattern.patternId(), start, end)));
        } catch (RegexpParserException exception) {
          return OracleResult.rejected(
              fixture,
              new Rejection(
                  "registration",
                  pattern.patternId(),
                  exception.getClass().getName(),
                  String.valueOf(exception.getMessage())));
        }
      }
      matcher.match(RMatch.stringBuffer(toString(fixture.input())));
    }
    events.sort(
        Comparator.comparingLong(MatchEvent::patternId)
            .thenComparingLong(MatchEvent::startUtf16)
            .thenComparingLong(MatchEvent::endUtf16));
    return OracleResult.matched(fixture, List.copyOf(events));
  }

  private static String toString(final int[] codeUnits) {
    final char[] chars = new char[codeUnits.length];
    for (int index = 0; index < codeUnits.length; index++) {
      chars[index] = (char) codeUnits[index];
    }
    return new String(chars);
  }

  private static void writeResults(final Path path, final List<OracleResult> results)
      throws IOException {
    createParent(path);
    try (BufferedWriter writer = Files.newBufferedWriter(path, StandardCharsets.UTF_8)) {
      for (final OracleResult result : results) {
        final StringWriter line = new StringWriter();
        try (JsonGenerator json = JSON.createGenerator(line)) {
          json.writeStartObject();
          json.writeNumberField("schema_version", SCHEMA_VERSION);
          json.writeStringField("case_id", result.fixture().caseId());
          json.writeStringField("rust_expectation", result.fixture().rustExpectation());
          json.writeStringField("status", result.status());
          if (result.rejection() == null) {
            json.writeArrayFieldStart("events");
            for (final MatchEvent event : result.events()) {
              json.writeStartObject();
              json.writeNumberField("pattern_id", event.patternId());
              json.writeNumberField("start_utf16", event.startUtf16());
              json.writeNumberField("end_utf16", event.endUtf16());
              json.writeEndObject();
            }
            json.writeEndArray();
          } else {
            json.writeObjectFieldStart("rejection");
            json.writeStringField("stage", result.rejection().stage());
            json.writeNumberField("pattern_id", result.rejection().patternId());
            json.writeStringField("exception_class", result.rejection().exceptionClass());
            json.writeStringField("message", result.rejection().message());
            json.writeEndObject();
          }
          json.writeEndObject();
        }
        writer.write(line.toString());
        writer.write('\n');
      }
    }
  }

  private static void writeManifest(
      final Path path,
      final String fixtureFile,
      final String fixtureSha256,
      final int caseCount,
      final String resultSha256)
      throws IOException {
    createParent(path);
    final StringWriter document = new StringWriter();
    try (JsonGenerator json = JSON.createGenerator(document)) {
      json.useDefaultPrettyPrinter();
      json.writeStartObject();
      json.writeNumberField("schema_version", SCHEMA_VERSION);
      json.writeStringField("fixture_file", fixtureFile);
      json.writeStringField("fixture_sha256", fixtureSha256);
      json.writeNumberField("case_count", caseCount);
      json.writeObjectFieldStart("oracle");
      json.writeStringField("group_id", ORACLE_GROUP);
      json.writeStringField("artifact_id", ORACLE_ARTIFACT);
      json.writeStringField("version", ORACLE_VERSION);
      json.writeStringField("jar_sha256", ORACLE_SHA256);
      json.writeNumberField("required_java_feature", REQUIRED_JAVA_FEATURE);
      json.writeEndObject();
      json.writeStringField("result_sha256", resultSha256);
      json.writeEndObject();
    }
    Files.writeString(
        path, document + "\n", StandardCharsets.UTF_8);
  }

  private static void createParent(final Path path) throws IOException {
    final Path parent = path.toAbsolutePath().getParent();
    if (parent != null) {
      Files.createDirectories(parent);
    }
  }

  private static String sha256(final Path path) throws IOException, NoSuchAlgorithmException {
    final MessageDigest digest = MessageDigest.getInstance("SHA-256");
    try (var input = Files.newInputStream(path)) {
      final byte[] chunk = new byte[8192];
      int read;
      while ((read = input.read(chunk)) != -1) {
        digest.update(chunk, 0, read);
      }
    }
    return HexFormat.of().formatHex(digest.digest());
  }

  private record FixtureCase(
      String caseId, String rustExpectation, List<FixturePattern> patterns, int[] input) {}

  private record FixturePattern(long patternId, int[] codeUnits) {}

  private record MatchEvent(long patternId, long startUtf16, long endUtf16) {}

  private record Rejection(
      String stage, long patternId, String exceptionClass, String message) {}

  private record OracleResult(
      FixtureCase fixture, String status, List<MatchEvent> events, Rejection rejection) {
    private static OracleResult matched(
        final FixtureCase fixture, final List<MatchEvent> events) {
      return new OracleResult(fixture, "matched", events, null);
    }

    private static OracleResult rejected(
        final FixtureCase fixture, final Rejection rejection) {
      return new OracleResult(fixture, "rejected", List.of(), rejection);
    }
  }

  private static final class FixtureFormatException extends IllegalArgumentException {
    private static final long serialVersionUID = 1L;

    private FixtureFormatException(final String message) {
      super(message);
    }
  }
}
