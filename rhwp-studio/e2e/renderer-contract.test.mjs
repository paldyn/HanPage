import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const studioRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const canvaskitPath = path.join(studioRoot, 'src/view/canvaskit-renderer.ts');
const canvaskitDirectory = path.join(studioRoot, 'src/view/canvaskit');
const layerTypesPath = path.join(studioRoot, 'src/core/types.ts');

const canvaskitSource = fs.readFileSync(canvaskitPath, 'utf8');
const layerTypesSource = fs.readFileSync(layerTypesPath, 'utf8');

function extractBlockBody(source, signatureIndex, blockName) {
  let bodyStart = -1;
  for (let index = signatureIndex; index < source.length; index += 1) {
    if (source[index] === '{') {
      bodyStart = index;
      break;
    }
  }
  assert.notEqual(bodyStart, -1, `missing body for ${blockName}`);

  let depth = 0;
  for (let index = bodyStart; index < source.length; index += 1) {
    const char = source[index];
    if (char === '{') {
      depth += 1;
    } else if (char === '}') {
      depth -= 1;
      if (depth === 0) {
        return source.slice(bodyStart + 1, index);
      }
    }
  }

  throw new Error(`unterminated body for ${blockName}`);
}

function extractMethodBody(source, methodName) {
  let signatureIndex = source.indexOf(`private ${methodName}(`);
  if (signatureIndex === -1) {
    signatureIndex = source.indexOf(`${methodName}(`);
  }
  assert.notEqual(signatureIndex, -1, `missing method ${methodName}`);

  return extractBlockBody(source, signatureIndex, methodName);
}

function caseLabels(methodBody) {
  return [...methodBody.matchAll(/case\s+'([^']+)':/g)].map((match) => match[1]);
}

function tsFilesUnder(directory) {
  return fs.readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        return tsFilesUnder(entryPath);
      }
      return entry.name.endsWith('.ts') ? [entryPath] : [];
    })
    .sort();
}

function layerPaintOpTypes() {
  const unionMatch = layerTypesSource.match(/export type LayerPaintOp =([\s\S]*?);/);
  assert.notEqual(unionMatch, null, 'missing LayerPaintOp union');
  const interfaceNames = [...unionMatch[1].matchAll(/\|\s*(Layer[A-Za-z0-9]+Op)\b/g)]
    .map((match) => match[1]);
  assert.ok(interfaceNames.length > 0, 'LayerPaintOp union has no variants');

  return interfaceNames.map((interfaceName) => {
    const interfacePattern = new RegExp(`export interface ${interfaceName} \\{[\\s\\S]*?type:\\s*'([^']+)'`);
    const interfaceMatch = layerTypesSource.match(interfacePattern);
    assert.notEqual(interfaceMatch, null, `missing literal type for ${interfaceName}`);
    return interfaceMatch[1];
  }).sort();
}

function layerNodeKinds() {
  const unionMatch = layerTypesSource.match(/export type LayerNode =([\s\S]*?);/);
  assert.notEqual(unionMatch, null, 'missing LayerNode union');
  const interfaceNames = unionMatch[1].split('|')
    .map((item) => item.trim().replace(/;$/, ''))
    .filter(Boolean);
  assert.ok(interfaceNames.length > 0, 'LayerNode union has no variants');

  return interfaceNames.map((interfaceName) => {
    const interfacePattern = new RegExp(`export interface ${interfaceName} \\{[\\s\\S]*?kind:\\s*'([^']+)'`);
    const interfaceMatch = layerTypesSource.match(interfacePattern);
    assert.notEqual(interfaceMatch, null, `missing kind literal for ${interfaceName}`);
    return interfaceMatch[1];
  }).sort();
}

function requireSnippet(source, pattern, message) {
  assert.match(source, pattern, message);
}

const renderOpBody = extractMethodBody(canvaskitSource, 'renderOp');
const renderNodeBody = extractMethodBody(canvaskitSource, 'renderNode');
const renderOpCases = caseLabels(renderOpBody).sort();
const layerOpTypes = layerPaintOpTypes();
const layerNodeKindSet = layerNodeKinds();
const canvaskitSourceFiles = [
  { label: path.relative(studioRoot, canvaskitPath), source: canvaskitSource },
  ...tsFilesUnder(canvaskitDirectory).map((filePath) => ({
    label: path.relative(studioRoot, filePath),
    source: fs.readFileSync(filePath, 'utf8'),
  })),
];
const forbiddenCanvas2dApiPatterns = [
  [/document\s*\.\s*createElement\b/, 'document.createElement'],
  [/\.getContext\s*\(/, 'HTMLCanvasElement.getContext'],
  [/\bCanvasRenderingContext2D\b/, 'CanvasRenderingContext2D'],
  [/\bPath2D\b/, 'Path2D'],
  [/\.measureText\s*\(/, 'CanvasRenderingContext2D.measureText'],
  [/\bOffscreenCanvas\b/, 'OffscreenCanvas'],
  [/\bImageData\b/, 'ImageData'],
  [/\bcreateImageBitmap\s*\(/, 'createImageBitmap'],
  [/\bImageBitmap\b/, 'ImageBitmap'],
  [/\bHTMLImageElement\b/, 'HTMLImageElement'],
  [/\bnew\s+Image\s*\(/, 'new Image'],
  [/\bDOMParser\b/, 'DOMParser'],
  [/\bXMLSerializer\b/, 'XMLSerializer'],
  [/\bURL\s*\.\s*createObjectURL\s*\(/, 'URL.createObjectURL'],
  [/\bFileReader\b/, 'FileReader'],
  [/\bCanvas2DLayerRenderer\b/, 'Canvas2DLayerRenderer'],
  [/canvas2d-layer-renderer/, 'canvas2d-layer-renderer import'],
];

assert.deepEqual(
  renderOpCases,
  layerOpTypes,
  'CanvasKit renderOp must explicitly mention every LayerPaintOp variant',
);
assert.deepEqual(
  layerNodeKindSet,
  ['clipRect', 'group', 'leaf'],
  'renderer contract guard should know every LayerNode kind',
);

requireSnippet(
  renderNodeBody,
  /node\.kind === 'group'[\s\S]*?for \(const child of node\.children\)[\s\S]*?this\.renderNode\(canvas, child,/,
  'group nodes should recurse through children',
);
requireSnippet(
  renderNodeBody,
  /node\.kind === 'clipRect'[\s\S]*?this\.renderClipNode\(canvas, node,/,
  'clipRect nodes should go through renderClipNode',
);
requireSnippet(
  renderNodeBody,
  /this\.renderLeaf\(canvas, node, replayPlane, activeLayer\);/,
  'leaf nodes should go through renderLeaf',
);

const directReplayOps = [
  'ellipse',
  'footnoteMarker',
  'formObject',
  'image',
  'line',
  'pageBackground',
  'path',
  'placeholder',
  'rectangle',
  'textRun',
];
const textRunFallbackOps = [
  'charOverlap',
  'equation',
  'glyphRun',
  'rawSvg',
  'tabLeader',
  'textControlMark',
  'textDecoration',
];

for (const op of directReplayOps) {
  assert.match(
    renderOpBody,
    new RegExp(`case '${op}':[\\s\\S]*?this\\.render[A-Za-z0-9]+\\(canvas,`),
    `${op} should dispatch to a CanvasKit replay method`,
  );
}

for (const op of textRunFallbackOps) {
  assert.match(
    renderOpBody,
    new RegExp(`case '${op}':[\\s\\S]*?this\\.unsupportedOps\\.add\\(op\\.type\\);[\\s\\S]*?return;`),
    `${op} should stay on the declared unsupported/TextRun fallback path`,
  );
}

requireSnippet(
  renderOpBody,
  /case 'glyphOutline':[\s\S]*?glyphOutlinePayloadStatus\(op,[\s\S]*?this\.renderGlyphOutline\(canvas, op\);[\s\S]*?this\.unsupportedOps\.add\(/,
  'glyphOutline should stay guarded by payload status before direct replay',
);

const renderRectangleBody = extractMethodBody(canvaskitSource, 'renderRectangle');
const renderEllipseBody = extractMethodBody(canvaskitSource, 'renderEllipse');
const renderPathBody = extractMethodBody(canvaskitSource, 'renderPath');
const renderLineBody = extractMethodBody(canvaskitSource, 'renderLine');
const renderFormObjectBody = extractMethodBody(canvaskitSource, 'renderFormObject');
const renderGlyphOutlineBody = extractMethodBody(canvaskitSource, 'renderGlyphOutline');
const renderColorPaintGraphNodeBody = extractMethodBody(canvaskitSource, 'renderColorPaintGraphNode');

requireSnippet(
  renderRectangleBody,
  /this\.drawStyledShape\(canvas, op\.bbox, op\.style,[\s\S]*?drawRRect[\s\S]*?drawRect/,
  'rectangle replay should stay on drawStyledShape and handle rounded and plain rectangles',
);
requireSnippet(
  renderEllipseBody,
  /this\.drawStyledShape\(canvas, op\.bbox, op\.style,[\s\S]*?drawOval/,
  'ellipse replay should stay on drawStyledShape',
);
requireSnippet(
  renderPathBody,
  /new this\.canvasKit\.Path\(\)[\s\S]*?this\.applyPathCommand[\s\S]*?this\.drawStyledPath/,
  'path replay should build CanvasKit paths through applyPathCommand and drawStyledPath',
);
requireSnippet(
  renderLineBody,
  /this\.makeStrokePaint\(op\.style\?\.color[\s\S]*?canvas\.drawLine\(op\.x1, op\.y1, op\.x2, op\.y2, paint\)/,
  'line replay should draw a CanvasKit line with stroke paint',
);
requireSnippet(
  renderFormObjectBody,
  /op\.formType === 'checkbox' \|\| op\.formType === 'radio'[\s\S]*?canvas\.drawLine[\s\S]*?const label = op\.caption \|\| op\.text[\s\S]*?this\.renderTextRun/,
  'form object replay should keep checkbox/radio mark and caption text branches explicit',
);
requireSnippet(
  renderGlyphOutlineBody,
  /op\.colorLayers\?\.paintGraph[\s\S]*?graph\.rootNodeId[\s\S]*?this\.renderColorPaintGraphNode/,
  'glyphOutline replay should require a colorLayers paint graph root',
);
requireSnippet(
  renderColorPaintGraphNodeBody,
  /visited\.has\(nodeId\)[\s\S]*?unsupportedColorGlyph[\s\S]*?node\.solidPath \?\? node\.linearGradientPath \?\? node\.radialGradientPath \?\? node\.sweepGradientPath/,
  'glyphOutline color graph replay should keep cycle guard and supported path families explicit',
);

for (const { label, source } of canvaskitSourceFiles) {
  for (const [pattern, name] of forbiddenCanvas2dApiPatterns) {
    assert.doesNotMatch(
      source,
      pattern,
      `CanvasKit direct replay source ${label} must not depend on ${name}`,
    );
  }
}

console.log('renderer backend contract guard passed');
