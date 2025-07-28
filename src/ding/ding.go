package main

import (
	"archive/zip"
	"bufio"
	"compress/gzip"
	"context"
	"embed"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"time"
)

//go:embed .env
var envFile embed.FS

type DepotInfo struct {
	DepotID       string   `json:"depot_id"`
	DecryptionKey string   `json:"decryption_key"`
	ManifestIDs   []string `json:"manifest_ids"`
}

type VDFNode struct {
	Value    string
	Children map[string]*VDFNode
}

type Config struct {
	CreateAppManifest bool
	AddAppIDToGoFile  bool
	ManifestCookie    string
	UserAgent         string
}

type ManifestDownloader struct {
	client        *http.Client
	baseDir       string
	githubAPI     string
	githubToken   string
	repoList      []string
	cnCDNList     []string
	globalCDNList []string
	isCN          bool
	config        Config
}

type RepoInfo struct {
	Name       string    `json:"name"`
	LastUpdate time.Time `json:"last_update"`
	SHA        string    `json:"sha"`
}

type BranchInfo struct {
	Commit struct {
		SHA    string `json:"sha"`
		Commit struct {
			Tree struct {
				SHA string `json:"sha"`
				URL string `json:"url"`
			} `json:"tree"`
			Author struct {
				Date string `json:"date"`
			} `json:"author"`
		} `json:"commit"`
	} `json:"commit"`
}

type TreeItem struct {
	Path string `json:"path"`
	Mode string `json:"mode"`
	Type string `json:"type"`
	SHA  string `json:"sha"`
	Size int    `json:"size"`
	URL  string `json:"url"`
}

type TreeResponse struct {
	SHA  string     `json:"sha"`
	Tree []TreeItem `json:"tree"`
}

// ManifestAPI response structures
type GameSearchResponse struct {
	Games []GameInfo `json:"games"`
}

type GameInfo struct {
	Name         string `json:"name"`
	LastModified string `json:"last_modified"`
	FileSize     int64  `json:"file_size"`
}

type PrepareDownloadResponse struct {
	Success        bool   `json:"success"`
	Message        string `json:"message"`
	DownloadToken  string `json:"download_token"`
	Filename       string `json:"filename"`
	FileSize       int64  `json:"file_size"`
	RemainingToday int    `json:"remaining_today"`
	ExpiresIn      int    `json:"expires_in"`
}

func (md *ManifestDownloader) loadEnv() {
	// 优先尝试读取嵌入的.env文件
	if content, err := envFile.ReadFile(".env"); err == nil {
		md.parseEnvContent(string(content))
		return
	}

	// 如果嵌入文件不存在，尝试读取本地.env文件
	envFile := ".env"
	file, err := os.Open(envFile)
	if err != nil {
		return // .env文件不存在，使用系统环境变量
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	var lines []string
	for scanner.Scan() {
		lines = append(lines, scanner.Text())
	}

	md.parseEnvContent(strings.Join(lines, "\n"))
}

func (md *ManifestDownloader) loadConfig() {
	configFile := "config.txt"
	content, err := os.ReadFile(configFile)
	if err != nil {
		// 默认配置
		md.config = Config{
			CreateAppManifest: true,
			AddAppIDToGoFile:  true,
			UserAgent:         "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36",
		}
		return
	}

	// 解析配置文件
	lines := strings.Split(string(content), "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}

		parts := strings.SplitN(line, "=", 2)
		if len(parts) == 2 {
			key := strings.TrimSpace(parts[0])
			value := strings.TrimSpace(parts[1])

			switch key {
			case "createAppManifest":
				md.config.CreateAppManifest = value == "1"
			case "addAppIDToGoFile":
				md.config.AddAppIDToGoFile = value == "1"
			case "MANIFEST_COOKIE":
				md.config.ManifestCookie = value
			case "USER_AGENT":
				md.config.UserAgent = value
			}
		}
	}
}

func (md *ManifestDownloader) parseEnvContent(content string) {
	lines := strings.Split(content, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}

		parts := strings.SplitN(line, "=", 2)
		if len(parts) == 2 {
			key := strings.TrimSpace(parts[0])
			value := strings.TrimSpace(parts[1])
			if key == "GITHUB_TOKEN" {
				md.githubToken = value
			}
		}
	}

	// Use environment variable as fallback if not set in config
	if md.githubToken == "" {
		md.githubToken = os.Getenv("GITHUB_TOKEN")
	}
}

func NewManifestDownloader() *ManifestDownloader {
	md := &ManifestDownloader{
		client: &http.Client{
			Timeout: 30 * time.Second,
		},
		baseDir:   "utils/ManifestHub",
		githubAPI: "https://api.github.com",
		repoList: []string{
			"ehgen0ng/ManifestHub",
			"SteamAutoCracks/ManifestHub",
			"Auiowu/ManifestAutoUpdate",
			"tymolu233/ManifestAutoUpdate-fix",
		},
		cnCDNList: []string{
			"https://cdn.jsdmirror.com/gh/{repo}@{sha}/{path}",
			"https://raw.gitmirror.com/{repo}/{sha}/{path}",
			"https://raw.dgithub.xyz/{repo}/{sha}/{path}",
			"https://gh.akass.cn/{repo}/{sha}/{path}",
		},
		globalCDNList: []string{
			"https://raw.githubusercontent.com/{repo}/{sha}/{path}",
		},
	}

	md.detectRegion()
	md.loadEnv()
	md.loadConfig()
	md.showTokenStatus()
	return md
}

func (md *ManifestDownloader) detectRegion() {
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	dialer := &net.Dialer{}
	conn, err := dialer.DialContext(ctx, "tcp", "google.com:80")
	if err != nil {
		md.isCN = true
		return
	}
	conn.Close()
	md.isCN = false
}

func (md *ManifestDownloader) showTokenStatus() {
	if md.githubToken == "" {
	}
}

func (md *ManifestDownloader) setAuthHeader(req *http.Request) {
	if md.githubToken != "" {
		req.Header.Set("Authorization", "Bearer "+md.githubToken)
	}
}

func (md *ManifestDownloader) checkLocalVersion(appID string) (*RepoInfo, error) {
	versionFile := filepath.Join(md.baseDir, appID, ".version")
	data, err := os.ReadFile(versionFile)
	if err != nil {
		return nil, err // 文件不存在或读取失败
	}

	var localRepo RepoInfo
	if err := json.Unmarshal(data, &localRepo); err != nil {
		return nil, err
	}

	return &localRepo, nil
}

func (md *ManifestDownloader) saveLocalVersion(appID string, repo *RepoInfo) error {
	versionFile := filepath.Join(md.baseDir, appID, ".version")
	data, err := json.MarshalIndent(repo, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(versionFile, data, 0644)
}

func (md *ManifestDownloader) getUserInput() (string, error) {
	fmt.Print("Enter the AppID: ")
	reader := bufio.NewReader(os.Stdin)
	input, err := reader.ReadString('\n')
	if err != nil {
		return "", fmt.Errorf("读取输入失败: %w", err)
	}

	appID := strings.TrimSpace(input)
	if appID == "" {
		return "", fmt.Errorf("AppID 不能为空")
	}

	if _, err := strconv.Atoi(appID); err != nil {
		return "", fmt.Errorf("AppID 必须是数字: %w", err)
	}

	return appID, nil
}

func (md *ManifestDownloader) createAppIDDir(appID string) error {
	dirPath := filepath.Join(md.baseDir, appID)
	err := os.MkdirAll(dirPath, 0755)
	if err != nil {
		return fmt.Errorf("创建目录失败 %s: %w", dirPath, err)
	}

	return nil
}

func (md *ManifestDownloader) getBranchInfo(ctx context.Context, repo, appID string) (*BranchInfo, error) {
	branchURL := fmt.Sprintf("%s/repos/%s/branches/%s", md.githubAPI, repo, appID)

	attempt := 1
	for {
		req, err := http.NewRequestWithContext(ctx, "GET", branchURL, nil)
		if err != nil {
			select {
			case <-time.After(time.Second):
			case <-ctx.Done():
				return nil, ctx.Err()
			}
			attempt++
			continue
		}

		md.setAuthHeader(req)
		req.Header.Set("User-Agent", md.config.UserAgent)

		// 添加User-Agent以避免GitHub阻止请求
		req.Header.Set("User-Agent", md.config.UserAgent)

		resp, err := md.client.Do(req)
		if err != nil {
			select {
			case <-time.After(time.Second):
			case <-ctx.Done():
				return nil, ctx.Err()
			}
			attempt++
			continue
		}
		defer resp.Body.Close()

		if resp.StatusCode == 404 {
			// 404说明分支不存在，直接返回错误，不重试
			bodyBytes, _ := io.ReadAll(resp.Body)
			return nil, fmt.Errorf("HTTP %d: %s", resp.StatusCode, string(bodyBytes))
		}

		if resp.StatusCode != 200 {
			select {
			case <-time.After(time.Second):
			case <-ctx.Done():
				return nil, ctx.Err()
			}
			attempt++
			continue
		}

		var branchInfo BranchInfo
		if err := json.NewDecoder(resp.Body).Decode(&branchInfo); err != nil {
			select {
			case <-time.After(time.Second):
			case <-ctx.Done():
				return nil, ctx.Err()
			}
			attempt++
			continue
		}

		return &branchInfo, nil
	}
}

func (md *ManifestDownloader) getFileListFromTree(ctx context.Context, treeURL string) ([]TreeItem, error) {
	attempt := 1
	for {
		req, err := http.NewRequestWithContext(ctx, "GET", treeURL, nil)
		if err != nil {
			select {
			case <-time.After(time.Second):
			case <-ctx.Done():
				return nil, ctx.Err()
			}
			attempt++
			continue
		}

		md.setAuthHeader(req)
		req.Header.Set("User-Agent", md.config.UserAgent)

		resp, err := md.client.Do(req)
		if err != nil {
			select {
			case <-time.After(time.Second):
			case <-ctx.Done():
				return nil, ctx.Err()
			}
			attempt++
			continue
		}
		defer resp.Body.Close()

		if resp.StatusCode != 200 {
			select {
			case <-time.After(time.Second):
			case <-ctx.Done():
				return nil, ctx.Err()
			}
			attempt++
			continue
		}

		var treeResp TreeResponse
		if err := json.NewDecoder(resp.Body).Decode(&treeResp); err != nil {
			select {
			case <-time.After(time.Second):
			case <-ctx.Done():
				return nil, ctx.Err()
			}
			attempt++
			continue
		}

		var files []TreeItem
		for _, item := range treeResp.Tree {
			if item.Type == "blob" && strings.ToLower(item.Path) != "readme.md" {
				files = append(files, item)
			}
		}

		fmt.Printf("✅ 通过 GitHub tree API 获取到 %d 个文件\n", len(files))
		return files, nil
	}
}

func (md *ManifestDownloader) findLatestRepo(ctx context.Context, appID string) (*RepoInfo, error) {
	var latestRepo *RepoInfo
	var lastError error

	for _, repo := range md.repoList {
		branchInfo, err := md.getBranchInfo(ctx, repo, appID)
		if err != nil {
			lastError = err
			continue
		}

		updateTime, err := time.Parse(time.RFC3339, branchInfo.Commit.Commit.Author.Date)
		if err != nil {
			fmt.Printf("⚠️  仓库 %s 无法解析更新时间\n", repo)
			lastError = err
			continue
		}

		currentRepo := &RepoInfo{
			Name:       repo,
			LastUpdate: updateTime,
			SHA:        branchInfo.Commit.SHA,
		}

		if latestRepo == nil || updateTime.After(latestRepo.LastUpdate) {
			latestRepo = currentRepo
		}
	}

	if latestRepo == nil {
		return nil, fmt.Errorf("找不到 AppID %s，%v", appID, lastError)
	}

	fmt.Printf("\n")
	fmt.Printf("🎯 %s (更新时间: %s, SHA: %s)\n",
		latestRepo.Name,
		latestRepo.LastUpdate.Format("2006-01-02 15:04:05"),
		latestRepo.SHA[:8])

	return latestRepo, nil
}

func (md *ManifestDownloader) downloadFileWithCDN(ctx context.Context, repo *RepoInfo, filePath string) ([]byte, error) {
	allCDNs := append(md.cnCDNList, md.globalCDNList...)

	// 根据地区优先选择CDN，但都会尝试
	var cdnList []string
	if md.isCN {
		cdnList = allCDNs // CN用户：先试CN CDN，再试全球CDN
	} else {
		// 全球用户：先试全球CDN，再试CN CDN
		cdnList = append(md.globalCDNList, md.cnCDNList...)
	}

	round := 1
	totalAttempts := 0
	for {
		for i, cdnTemplate := range cdnList {
			totalAttempts++
			url := strings.ReplaceAll(cdnTemplate, "{repo}", repo.Name)
			url = strings.ReplaceAll(url, "{sha}", repo.SHA)
			url = strings.ReplaceAll(url, "{path}", filePath)

			// 第一次尝试每个CDN时不显示，重试时显示
			if totalAttempts > len(cdnList) {
				fmt.Printf("\r🔄 重试第%d轮 CDN%d/%d: %s", round-1, i+1, len(cdnList), filepath.Base(filePath))
			}

			req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
			if err != nil {
				continue
			}

			req.Header.Set("User-Agent", md.config.UserAgent)

			resp, err := md.client.Do(req)
			if err != nil {
				continue
			}

			if resp.StatusCode == 200 {
				data, err := io.ReadAll(resp.Body)
				resp.Body.Close()
				if err == nil {
					// 成功时清除重试信息（如果有的话）
					if totalAttempts > len(cdnList) {
						fmt.Printf("\r                                        \r")
					}
					return data, nil
				}
			}
			resp.Body.Close()
		}

		// 一轮CDN都失败了，等待1秒后继续下一轮
		select {
		case <-time.After(time.Second):
		case <-ctx.Done():
			if totalAttempts > len(cdnList) {
				fmt.Printf("\r❌ 下载被取消: %s\n", filepath.Base(filePath))
			}
			return nil, ctx.Err()
		}
		round++
	}
}

func (md *ManifestDownloader) downloadAllFiles(ctx context.Context, appID string, repo *RepoInfo) error {
	fmt.Printf("📥 开始为 AppID %s 下载清单文件...\n", appID)

	appDir := filepath.Join(md.baseDir, appID)

	// 检查本地版本
	localRepo, err := md.checkLocalVersion(appID)
	needsUpdate := false

	if err != nil {
		fmt.Println("📦 首次下载，获取所有文件")
		needsUpdate = true
	} else if localRepo.SHA != repo.SHA {
		fmt.Printf("🔄 检测到更新: %s -> %s\n", localRepo.SHA[:8], repo.SHA[:8])
		needsUpdate = true
	} else {
		fmt.Printf("\n")
		fmt.Printf("✅ 版本已是最新 (SHA: %s)\n", repo.SHA[:8])
		fmt.Printf("📋 继续检查密钥文件...\n")
	}

	branchInfo, err := md.getBranchInfo(ctx, repo.Name, appID)
	if err != nil {
		return fmt.Errorf("获取分支信息失败: %w", err)
	}

	fileList, err := md.getFileListFromTree(ctx, branchInfo.Commit.Commit.Tree.URL)
	if err != nil {
		return fmt.Errorf("获取文件列表失败: %w", err)
	}

	downloadedCount := 0
	for _, file := range fileList {
		filePath := filepath.Join(appDir, file.Path)

		// 如果需要更新或文件不存在，则下载
		if !needsUpdate {
			if _, err := os.Stat(filePath); err == nil {
				fmt.Printf("✅ 文件已存在: %s\n", file.Path)
				continue
			}
		}

		data, err := md.downloadFileWithCDN(ctx, repo, file.Path)
		if err != nil {
			fmt.Printf("⚠️  文件下载失败: %s - %v\n", file.Path, err)
			continue
		}

		if err := os.MkdirAll(filepath.Dir(filePath), 0755); err != nil {
			fmt.Printf("⚠️  创建子目录失败: %s - %v\n", filepath.Dir(filePath), err)
			continue
		}

		err = os.WriteFile(filePath, data, 0644)
		if err != nil {
			fmt.Printf("⚠️  保存文件失败: %s - %v\n", file.Path, err)
			continue
		}

		fmt.Printf("✅ 文件已保存: %s\n", filePath)
		downloadedCount++
	}

	// 保存版本信息
	if err := md.saveLocalVersion(appID, repo); err != nil {
		fmt.Printf("⚠️  保存版本信息失败: %v\n", err)
	} else {
		fmt.Printf("💾 版本信息已保存: %s\n", repo.SHA[:8])
	}

	fmt.Printf("📊 成功下载 %d 个文件\n", downloadedCount)

	// 查找并解析key.vdf文件
	if err := md.processDepotKeys(appID); err != nil {
		fmt.Printf("⚠️  处理depot密钥失败: %v\n", err)
	}

	return nil
}

func (md *ManifestDownloader) Run() error {
	// 首先检查本地ZIP文件
	zipFiles, err := md.checkLocalZipFiles()
	if err == nil && len(zipFiles) > 0 {
		// 处理找到的ZIP文件
		for _, zipPath := range zipFiles {
			// 从文件名提取AppID
			appID, err := md.extractAppIDFromZipName(zipPath)
			if err != nil {
				continue
			}

			// 解压ZIP文件到ManifestHub目录
			if err := md.extractZipToManifestDir(zipPath, appID); err != nil {
				continue
			}

			// 检查解压后的目录是否包含密钥文件
			appDir := filepath.Join(md.baseDir, appID)
			if !md.hasKeyFiles(appDir) {
				continue
			}

			// 直接处理密钥文件
			fmt.Printf("🎯 开始处理ZIP文件: %s (AppID: %s)\n", filepath.Base(zipPath), appID)
			if err := md.processDepotKeys(appID); err != nil {
				continue
			}

			// 删除处理成功的ZIP文件
			os.Remove(zipPath)

			return nil // 成功处理一个ZIP文件后返回
		}
	}

	// 原有流程：用户输入AppID
	appID, err := md.getUserInput()
	if err != nil {
		return fmt.Errorf("输入错误: %w", err)
	}

	// 首先尝试使用Manifest API下载
	success, err := md.downloadWithManifestAPI(appID)
	if success {
		// Manifest API下载成功，直接返回
		return nil
	} else if err != nil {
		// 有配置cookie但下载失败，显示错误信息然后fallback
		fmt.Printf("❌ Manifest API下载失败: %v\n", err)
	}

	// 使用原有的GitHub方法下载
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	repo, err := md.findLatestRepo(ctx, appID)
	if err != nil {
		return fmt.Errorf("查找仓库失败: %w", err)
	}

	if err := md.createAppIDDir(appID); err != nil {
		return fmt.Errorf("目录创建失败: %w", err)
	}

	if err := md.downloadAllFiles(ctx, appID, repo); err != nil {
		return fmt.Errorf("下载文件失败: %w", err)
	}

	return nil
}

func main() {
	downloader := NewManifestDownloader()

	if err := downloader.Run(); err != nil {
		fmt.Printf("❌ 错误: %v\n", err)
	}

	fmt.Print("\n按回车键退出...")
	bufio.NewReader(os.Stdin).ReadLine()
}

func parseVDF(content string) (*VDFNode, error) {
	content = strings.TrimSpace(content)
	lines := strings.Split(content, "\n")

	root := &VDFNode{Children: make(map[string]*VDFNode)}
	stack := []*VDFNode{root}

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "//") {
			continue
		}

		if line == "{" {
			continue
		}

		if line == "}" {
			if len(stack) > 1 {
				stack = stack[:len(stack)-1]
			}
			continue
		}

		// 解析键值对
		parts := parseVDFLine(line)
		if len(parts) >= 1 {
			key := parts[0]
			current := stack[len(stack)-1]

			if len(parts) == 1 || parts[1] == "" {
				// 这是一个节点声明，下一行应该是 {
				node := &VDFNode{Children: make(map[string]*VDFNode)}
				current.Children[key] = node
				stack = append(stack, node)
			} else {
				// 这是一个键值对
				current.Children[key] = &VDFNode{Value: parts[1]}
			}
		}
	}

	return root, nil
}

func parseVDFLine(line string) []string {
	re := regexp.MustCompile(`"([^"]*)"`)
	matches := re.FindAllStringSubmatch(line, -1)

	var parts []string
	for _, match := range matches {
		if len(match) > 1 {
			parts = append(parts, match[1])
		}
	}

	// 如果只有一个引号部分，说明这是一个节点声明
	if len(parts) == 1 {
		parts = append(parts, "")
	}

	return parts
}

func (vdf *VDFNode) String() string {
	return vdf.stringifyNode(0)
}

func (vdf *VDFNode) stringifyNode(indent int) string {
	var result strings.Builder
	indentStr := strings.Repeat("\t", indent)

	// 先输出所有的键值对
	for key, child := range vdf.Children {
		if child.Value != "" {
			result.WriteString(fmt.Sprintf("%s\"%s\"\t\t\"%s\"\n", indentStr, key, child.Value))
		}
	}

	// 再输出所有的子节点
	for key, child := range vdf.Children {
		if child.Value == "" {
			result.WriteString(fmt.Sprintf("%s\"%s\"\n", indentStr, key))
			result.WriteString(fmt.Sprintf("%s{\n", indentStr))
			result.WriteString(child.stringifyNode(indent + 1))
			result.WriteString(fmt.Sprintf("%s}\n", indentStr))
		}
	}

	return result.String()
}

func (md *ManifestDownloader) parseKeyVDF(content []byte) ([]DepotInfo, error) {
	vdfContent := string(content)
	root, err := parseVDF(vdfContent)
	if err != nil {
		return nil, fmt.Errorf("解析VDF失败: %w", err)
	}

	var depots []DepotInfo

	// 查找depots节点
	if depotsNode, exists := root.Children["depots"]; exists {
		for depotID, depotNode := range depotsNode.Children {
			if keyNode, exists := depotNode.Children["DecryptionKey"]; exists {
				depot := DepotInfo{
					DepotID:       depotID,
					DecryptionKey: keyNode.Value,
				}
				depots = append(depots, depot)
			}
		}
	}

	return depots, nil
}

func (md *ManifestDownloader) parseAppIDLua(content []byte) ([]DepotInfo, error) {
	luaContent := string(content)
	lines := strings.Split(luaContent, "\n")

	var depots []DepotInfo
	// 只匹配带密钥的格式: addappid(id,flag,"hash")
	re := regexp.MustCompile(`addappid\((\d+)(?:,\s*[01])?,\s*"([a-fA-F0-9]+)"\)`)

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "--") || line == "" {
			continue
		}

		matches := re.FindStringSubmatch(line)
		if len(matches) == 3 {
			depotID := matches[1]
			decryptionKey := matches[2]

			depot := DepotInfo{
				DepotID:       depotID,
				DecryptionKey: decryptionKey,
			}
			depots = append(depots, depot)
		}
	}

	return depots, nil
}

func (md *ManifestDownloader) addDepotKeysToConfig(configPath string, depots []DepotInfo) error {
	// 读取现有配置
	content, err := os.ReadFile(configPath)
	if err != nil {
		return fmt.Errorf("读取config.vdf失败: %w", err)
	}

	configStr := string(content)

	// 查找 "depots" 节点的位置
	depotsStart := strings.Index(configStr, `"depots"`)
	if depotsStart == -1 {
		return fmt.Errorf("未找到depots节点")
	}

	// 找到 depots 节点的开始 {
	openBracePos := strings.Index(configStr[depotsStart:], "{")
	if openBracePos == -1 {
		return fmt.Errorf("未找到depots节点")
	}
	openBracePos += depotsStart

	// 找到对应的结束 }
	braceCount := 0
	depotsEnd := -1
	for i := openBracePos; i < len(configStr); i++ {
		if configStr[i] == '{' {
			braceCount++
		} else if configStr[i] == '}' {
			braceCount--
			if braceCount == 0 {
				depotsEnd = i
				break
			}
		}
	}

	if depotsEnd == -1 {
		return fmt.Errorf("未找到depots节点")
	}

	updatedConfig := configStr
	addedCount := 0
	updatedCount := 0

	for _, depot := range depots {
		// 检查是否已存在此depot
		depotPattern := fmt.Sprintf(`"%s"`, depot.DepotID)
		depotsSection := updatedConfig[openBracePos+1 : depotsEnd]

		if strings.Contains(depotsSection, depotPattern) {
			// 存在则删除原有条目
			// 找到这个depot的开始位置（在depots节点内查找）
			searchStart := openBracePos + 1
			depotStart := strings.Index(updatedConfig[searchStart:depotsEnd], depotPattern)
			if depotStart != -1 {
				depotStart += searchStart

				// 找到这个depot的开始行（包含换行符）
				lineStart := depotStart
				for lineStart > 0 && updatedConfig[lineStart-1] != '\n' {
					lineStart--
				}

				// 找到这个depot条目的结束位置（包含结束大括号）
				depotBraceCount := 0
				depotEnd := -1
				inDepot := false

				for i := depotStart; i < depotsEnd; i++ {
					if updatedConfig[i] == '{' {
						depotBraceCount++
						inDepot = true
					} else if updatedConfig[i] == '}' {
						depotBraceCount--
						if inDepot && depotBraceCount == 0 {
							// 找到结束大括号后的换行符
							depotEnd = i + 1
							if depotEnd < len(updatedConfig) && updatedConfig[depotEnd] == '\n' {
								depotEnd++
							}
							break
						}
					}
				}

				if depotEnd != -1 {
					// 删除原有条目（从行开始到行结束）
					updatedConfig = updatedConfig[:lineStart] + updatedConfig[depotEnd:]
					// 重新计算 depotsEnd 位置
					adjustment := depotEnd - lineStart
					depotsEnd -= adjustment
					updatedCount++
				}
			}
		}

		// 添加新的depot条目
		newDepotEntry := fmt.Sprintf("\t\t\t\t\t\"%s\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"DecryptionKey\"\t\t\"%s\"\n\t\t\t\t\t}\n", depot.DepotID, depot.DecryptionKey)
		updatedConfig = updatedConfig[:depotsEnd] + newDepotEntry + updatedConfig[depotsEnd:]

		// 更新 depotsEnd 位置
		depotsEnd += len(newDepotEntry)
		addedCount++

		fmt.Printf("📝 添加depot密钥: %s -> %s\n", depot.DepotID, depot.DecryptionKey[:16]+"...")
	}

	// 写回文件
	err = os.WriteFile(configPath, []byte(updatedConfig), 0644)
	if err != nil {
		return fmt.Errorf("写入config.vdf失败: %w", err)
	}

	if updatedCount > 0 {
		fmt.Printf("✅ 已更新 %d 个现有depot密钥，添加 %d 个新depot密钥到 %s\n", updatedCount, addedCount-updatedCount, configPath)
	} else {
		fmt.Printf("✅ 已添加 %d 个depot密钥到 %s\n", addedCount, configPath)
	}
	return nil
}

func (md *ManifestDownloader) processDepotKeys(appID string) error {
	appDir := filepath.Join(md.baseDir, appID)

	// 优先查找任意lua文件
	luaPattern := filepath.Join(appDir, "*.lua")
	luaFiles, _ := filepath.Glob(luaPattern)
	var luaFilePath string

	if len(luaFiles) > 0 {
		luaFilePath = luaFiles[0] // 使用找到的第一个lua文件
	}

	var depots []DepotInfo

	// 优先尝试解析Lua文件
	if luaFilePath != "" {
		fmt.Printf("🔑 找到Lua密钥文件: %s\n", luaFilePath)

		content, err := os.ReadFile(luaFilePath)
		if err != nil {
			return fmt.Errorf("读取Lua密钥文件失败: %w", err)
		}

		depots, err = md.parseAppIDLua(content)
		if err == nil && len(depots) > 0 {
			// Lua解析成功，直接使用结果
			fmt.Printf("🔓 从Lua文件解析到 %d 个depot密钥\n", len(depots))
		} else {
			fmt.Printf("⚠️  Lua解析失败或无有效密钥，尝试解析VDF文件: %v\n", err)
			depots = nil // 清空结果，准备尝试VDF
		}
	}

	// 如果Lua解析失败或没有Lua文件，尝试VDF文件
	if len(depots) == 0 {
		keyFilePath := filepath.Join(appDir, "key.vdf")
		if _, err := os.Stat(keyFilePath); err != nil {
			return fmt.Errorf("未找到key.vdf文件")
		}

		fmt.Printf("🔑 找到VDF密钥文件: %s\n", keyFilePath)

		content, err := os.ReadFile(keyFilePath)
		if err != nil {
			return fmt.Errorf("读取密钥文件失败: %w", err)
		}

		depots, err = md.parseKeyVDF(content)
		if err != nil {
			return fmt.Errorf("解析密钥文件失败: %w", err)
		}
	}

	if len(depots) == 0 {
		return fmt.Errorf("未找到有效的depot密钥")
	}

	fmt.Printf("🔓 解析到 %d 个depot密钥\n", len(depots))

	// 查找Steam目录并备份config.vdf
	steamPath := getSteamPathFromRegistry()
	if steamPath == "" {
		fmt.Println("⚠️  未找到Steam安装路径")
		return nil
	}

	configPath := filepath.Join(steamPath, "config", "config.vdf")
	fmt.Printf("🎯 找到Steam配置文件: %s\n", configPath)

	// 备份config.vdf
	if err := md.backupSteamConfig(configPath); err != nil {
		fmt.Printf("⚠️  备份配置文件失败: %v\n", err)
	}

	// 修改Steam的config.vdf
	if err := md.addDepotKeysToConfig(configPath, depots); err != nil {
		return fmt.Errorf("添加密钥到Steam配置文件失败: %w", err)
	}

	// 创建AppID.txt文件，保存depotID列表
	if err := md.createDepotIDFile(appID, depots); err != nil {
		fmt.Printf("⚠️  创建depotID文件失败: %v\n", err)
	}

	// 复制.manifest文件到Steam的depotcache目录
	if err := md.copyManifestFiles(appID, steamPath); err != nil {
		fmt.Printf("⚠️  复制manifest文件失败: %v\n", err)
	}

	// 创建appmanifest文件到Steam的steamapps目录
	if md.config.CreateAppManifest {
		if err := md.createAppManifest(appID, steamPath); err != nil {
			fmt.Printf("⚠️  创建appmanifest文件失败: %v\n", err)
		}
	}

	// 无论appmanifest文件是否已存在，都确保AppID被添加到go.txt
	if md.config.AddAppIDToGoFile {
		if err := md.addAppIDToGoFile(appID); err != nil {
		}
	}

	return nil
}

func (md *ManifestDownloader) fetchAppInfoWithRetry(appID string) string {
	maxRetries := 5
	retryDelay := 2 * time.Second

	for attempt := 1; attempt <= maxRetries; attempt++ {
		installdir := md.fetchAppInfo(appID)
		if installdir != "" {
			return installdir
		}

		if attempt < maxRetries {
			time.Sleep(retryDelay)
		}
	}

	return ""
}

func (md *ManifestDownloader) fetchAppInfo(appID string) string {
	url := fmt.Sprintf("https://steamui.com/api/get_appinfo.php?appid=%s", appID)

	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return ""
	}

	req.Header.Set("User-Agent", md.config.UserAgent)

	resp, err := md.client.Do(req)
	if err != nil {
		return ""
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		return ""
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return ""
	}

	responseStr := string(body)

	// 直接用正则表达式查找installdir字段
	re := regexp.MustCompile(`"installdir"\s+"([^"]+)"`)
	matches := re.FindStringSubmatch(responseStr)

	if len(matches) >= 2 {
		return matches[1]
	}

	return ""
}

func (md *ManifestDownloader) createAppManifest(appID, steamPath string) error {
	steamAppsDir := filepath.Join(steamPath, "steamapps")

	// 创建steamapps目录（如果不存在）
	if err := os.MkdirAll(steamAppsDir, 0755); err != nil {
		return fmt.Errorf("创建steamapps目录失败: %w", err)
	}

	// 生成appmanifest文件名
	manifestFileName := fmt.Sprintf("appmanifest_%s.acf", appID)
	manifestPath := filepath.Join(steamAppsDir, manifestFileName)

	// 检查文件是否已存在
	if _, err := os.Stat(manifestPath); err == nil {
		fmt.Printf("✅ appmanifest文件已存在: %s\n", manifestPath)
		return nil
	}

	// 获取installdir
	installdir := md.fetchAppInfoWithRetry(appID)
	if installdir == "" {
		// 如果获取installdir失败，跳过文件生成
		return nil
	}

	// 创建appmanifest内容
	content := fmt.Sprintf(`"AppState"
{
	"appid"		"%s"
	"Universe"		"1"
	"StateFlags"		"2"
	"installdir"		"%s"
}
`, appID, installdir)

	// 写入文件
	err := os.WriteFile(manifestPath, []byte(content), 0644)
	if err != nil {
		return fmt.Errorf("写入appmanifest文件失败: %w", err)
	}

	fmt.Printf("📄 已创建appmanifest文件: %s\n", manifestPath)
	return nil
}

func (md *ManifestDownloader) addAppIDToGoFile(appID string) error {
	listDir := "List"
	if err := os.MkdirAll(listDir, 0755); err != nil {
		return fmt.Errorf("创建List目录失败: %w", err)
	}

	goFile := filepath.Join(listDir, "go.txt")

	// 检查AppID是否已存在
	if md.isAppIDInGoFile(goFile, appID) {
		return nil
	}

	// 添加到go.txt
	file, err := os.OpenFile(goFile, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
	if err != nil {
		return fmt.Errorf("打开go.txt文件失败: %w", err)
	}
	defer file.Close()

	if _, err := file.WriteString(appID + "\n"); err != nil {
		return fmt.Errorf("写入go.txt文件失败: %w", err)
	}

	return nil
}

func (md *ManifestDownloader) isAppIDInGoFile(filePath, appID string) bool {
	content, err := os.ReadFile(filePath)
	if err != nil {
		return false
	}

	lines := strings.Split(string(content), "\n")
	for _, line := range lines {
		if strings.TrimSpace(line) == appID {
			return true
		}
	}
	return false
}

func (md *ManifestDownloader) createDepotIDFile(appID string, depots []DepotInfo) error {
	appDir := filepath.Join(md.baseDir, appID)
	depotIDFile := filepath.Join(appDir, appID+".txt")

	var depotIDs []string
	for _, depot := range depots {
		depotIDs = append(depotIDs, depot.DepotID)
	}

	content := strings.Join(depotIDs, "\n")
	if content != "" {
		content += "\n" // 在最后添加换行符
	}

	err := os.WriteFile(depotIDFile, []byte(content), 0644)
	if err != nil {
		return fmt.Errorf("写入depotID文件失败: %w", err)
	}

	fmt.Printf("📝 已创建depotID文件: %s (包含 %d 个depot)\n", depotIDFile, len(depotIDs))
	return nil
}

func (md *ManifestDownloader) copyManifestFiles(appID, steamPath string) error {
	appDir := filepath.Join(md.baseDir, appID)
	depotCacheDir := filepath.Join(steamPath, "depotcache")

	// 创建depotcache目录（如果不存在）
	if err := os.MkdirAll(depotCacheDir, 0755); err != nil {
		return fmt.Errorf("创建depotcache目录失败: %w", err)
	}

	// 查找所有.manifest文件
	manifestFiles, err := filepath.Glob(filepath.Join(appDir, "*.manifest"))
	if err != nil {
		return fmt.Errorf("查找manifest文件失败: %w", err)
	}

	if len(manifestFiles) == 0 {
		fmt.Println("📁 未找到manifest文件，跳过复制")
		return nil
	}

	fmt.Printf("📤 开始复制 %d 个manifest文件到depotcache...\n", len(manifestFiles))

	copiedCount := 0
	for _, manifestFile := range manifestFiles {
		fileName := filepath.Base(manifestFile)
		destPath := filepath.Join(depotCacheDir, fileName)

		// 复制文件
		srcFile, err := os.Open(manifestFile)
		if err != nil {
			fmt.Printf("⚠️  打开源文件失败 %s: %v\n", fileName, err)
			continue
		}

		dstFile, err := os.Create(destPath)
		if err != nil {
			srcFile.Close()
			fmt.Printf("⚠️  创建目标文件失败 %s: %v\n", fileName, err)
			continue
		}

		_, err = io.Copy(dstFile, srcFile)
		srcFile.Close()
		dstFile.Close()

		if err != nil {
			fmt.Printf("⚠️  复制文件失败 %s: %v\n", fileName, err)
			continue
		}

		fmt.Printf("✅ 已复制: %s -> %s\n", fileName, depotCacheDir)
		copiedCount++
	}

	fmt.Printf("📊 成功复制 %d/%d 个manifest文件到depotcache\n", copiedCount, len(manifestFiles))
	return nil
}

func getSteamPathFromRegistry() string {
	// 尝试从用户注册表获取
	if path := queryRegistry("HKCU", "SOFTWARE\\Valve\\Steam", "SteamPath"); path != "" {
		return path
	}

	// 尝试从系统注册表获取
	if path := queryRegistry("HKLM", "SOFTWARE\\WOW6432Node\\Valve\\Steam", "InstallPath"); path != "" {
		return path
	}

	return ""
}

func queryRegistry(hive, key, value string) string {
	cmd := exec.Command("reg", "query", hive+"\\"+key, "/v", value)
	output, err := cmd.Output()
	if err != nil {
		return ""
	}

	lines := strings.Split(string(output), "\n")
	for _, line := range lines {
		if strings.Contains(line, value) {
			parts := strings.Fields(line)
			if len(parts) >= 3 {
				// 获取路径部分（可能包含空格）
				pathStart := strings.Index(line, "REG_SZ") + 6
				if pathStart > 6 && pathStart < len(line) {
					path := strings.TrimSpace(line[pathStart:])
					return strings.ReplaceAll(path, "/", "\\")
				}
			}
		}
	}

	return ""
}

func (md *ManifestDownloader) backupSteamConfig(configPath string) error {
	// 检查配置文件是否存在
	if _, err := os.Stat(configPath); err != nil {
		return fmt.Errorf("Steam配置文件不存在: %s", configPath)
	}

	// 创建备份目录
	backupDir := filepath.Join(md.baseDir, "backup")
	if err := os.MkdirAll(backupDir, 0755); err != nil {
		return fmt.Errorf("创建备份目录失败: %w", err)
	}

	// 生成时间戳
	timestamp := time.Now().Format("20060102_150405")
	backupPath := filepath.Join(backupDir, fmt.Sprintf("config_%s.vdf", timestamp))

	// 复制文件
	srcFile, err := os.Open(configPath)
	if err != nil {
		return fmt.Errorf("打开配置文件失败: %w", err)
	}
	defer srcFile.Close()

	dstFile, err := os.Create(backupPath)
	if err != nil {
		return fmt.Errorf("创建备份文件失败: %w", err)
	}
	defer dstFile.Close()

	if _, err := io.Copy(dstFile, srcFile); err != nil {
		return fmt.Errorf("复制文件失败: %w", err)
	}

	fmt.Printf("💾 已备份Steam配置文件到: %s\n", backupPath)
	return nil
}

// checkLocalZipFiles 检测当前目录下的ZIP文件
func (md *ManifestDownloader) checkLocalZipFiles() ([]string, error) {
	// 获取当前执行文件的目录
	execPath, err := os.Executable()
	if err != nil {
		return nil, fmt.Errorf("获取执行文件路径失败: %w", err)
	}
	execDir := filepath.Dir(execPath)

	// 查找所有ZIP文件
	pattern := filepath.Join(execDir, "*.zip")
	zipFiles, err := filepath.Glob(pattern)
	if err != nil {
		return nil, fmt.Errorf("查找ZIP文件失败: %w", err)
	}

	return zipFiles, nil
}

// extractAppIDFromZipName 从ZIP文件名提取AppID
func (md *ManifestDownloader) extractAppIDFromZipName(zipPath string) (string, error) {
	fileName := filepath.Base(zipPath)
	// 移除.zip扩展名
	nameWithoutExt := strings.TrimSuffix(fileName, ".zip")

	// 使用正则表达式提取数字部分
	re := regexp.MustCompile(`^(\d+)`)
	matches := re.FindStringSubmatch(nameWithoutExt)

	if len(matches) < 2 {
		return "", fmt.Errorf("无法从文件名 %s 提取AppID", fileName)
	}

	appID := matches[1]
	// 验证AppID是否为有效数字
	if _, err := strconv.Atoi(appID); err != nil {
		return "", fmt.Errorf("提取的AppID %s 不是有效数字", appID)
	}

	return appID, nil
}

// extractZipToManifestDir 解压ZIP文件到ManifestHub目录
func (md *ManifestDownloader) extractZipToManifestDir(zipPath, appID string) error {
	// 创建目标目录
	targetDir := filepath.Join(md.baseDir, appID)
	if err := os.MkdirAll(targetDir, 0755); err != nil {
		return fmt.Errorf("创建目标目录失败: %w", err)
	}

	// 打开ZIP文件
	reader, err := zip.OpenReader(zipPath)
	if err != nil {
		return fmt.Errorf("打开ZIP文件失败: %w", err)
	}
	defer reader.Close()

	extractedCount := 0
	for _, file := range reader.File {
		// 构建目标文件路径
		destPath := filepath.Join(targetDir, file.Name)

		// 确保路径安全（防止zip slip攻击）
		if !strings.HasPrefix(destPath, filepath.Clean(targetDir)+string(os.PathSeparator)) {
			continue
		}

		if file.FileInfo().IsDir() {
			// 创建目录
			if err := os.MkdirAll(destPath, file.FileInfo().Mode()); err != nil {
				continue
			}
		} else {
			// 解压文件
			if err := md.extractZipFile(file, destPath); err != nil {
				continue
			}
			extractedCount++
		}
	}

	if extractedCount == 0 {
		return fmt.Errorf("未解压任何文件")
	}

	return nil
}

// extractZipFile 解压单个文件
func (md *ManifestDownloader) extractZipFile(file *zip.File, destPath string) error {
	// 创建目标目录
	if err := os.MkdirAll(filepath.Dir(destPath), 0755); err != nil {
		return err
	}

	// 打开ZIP文件中的文件
	srcFile, err := file.Open()
	if err != nil {
		return err
	}
	defer srcFile.Close()

	// 创建目标文件
	dstFile, err := os.Create(destPath)
	if err != nil {
		return err
	}
	defer dstFile.Close()

	// 复制内容
	_, err = io.Copy(dstFile, srcFile)
	return err
}

// hasKeyFiles 检查目录是否包含密钥文件
func (md *ManifestDownloader) hasKeyFiles(appDir string) bool {
	// 检查lua文件
	luaPattern := filepath.Join(appDir, "*.lua")
	luaFiles, _ := filepath.Glob(luaPattern)
	if len(luaFiles) > 0 {
		return true
	}

	// 检查key.vdf文件
	keyFile := filepath.Join(appDir, "key.vdf")
	if _, err := os.Stat(keyFile); err == nil {
		return true
	}

	return false
}

// 读取HTTP响应并处理gzip解压缩
func (md *ManifestDownloader) readResponseBody(resp *http.Response) ([]byte, error) {
	var reader io.Reader = resp.Body

	// 检查是否是 gzip 压缩
	if resp.Header.Get("Content-Encoding") == "gzip" {
		gzipReader, err := gzip.NewReader(resp.Body)
		if err != nil {
			return nil, fmt.Errorf("gzip解压缩失败: %v", err)
		}
		defer gzipReader.Close()
		reader = gzipReader
	}

	return io.ReadAll(reader)
}

// 搜索游戏信息
func (md *ManifestDownloader) searchGame(appID string) (*GameInfo, error) {
	baseURL := "https://manifest.morrenus.xyz"
	searchURL := fmt.Sprintf("%s/api/games?limit=100&offset=0&search=%s&platform=&status=&dlc_status=&genre=&sort_by=date_newest", baseURL, appID)

	req, err := http.NewRequest("GET", searchURL, nil)
	if err != nil {
		return nil, fmt.Errorf("❌ 创建请求失败: %v", err)
	}

	// 设置请求头
	req.Header.Set("Accept", "*/*")
	req.Header.Set("User-Agent", md.config.UserAgent)
	req.Header.Set("Accept-Language", "zh-CN,zh-Hans;q=0.9")
	req.Header.Set("Accept-Encoding", "gzip, deflate")
	req.Header.Set("Referer", baseURL+"/")

	if md.config.ManifestCookie != "" {
		req.Header.Set("Cookie", strings.TrimSpace(md.config.ManifestCookie))
	}

	resp, err := md.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("❌ 请求失败: %v", err)
	}
	defer resp.Body.Close()

	body, err := md.readResponseBody(resp)
	if err != nil {
		return nil, fmt.Errorf("❌ 读取响应失败: %v", err)
	}

	// 检查是否是HTML错误页面
	bodyStr := string(body)
	if strings.Contains(bodyStr, "<!DOCTYPE html>") {
		return nil, fmt.Errorf("❌ 服务器返回HTML错误页面")
	}

	var searchResp GameSearchResponse
	if err := json.Unmarshal(body, &searchResp); err != nil {
		fmt.Printf("❌ 服务器响应: %s\n", bodyStr)
		return nil, fmt.Errorf("❌ JSON解析失败: %v", err)
	}

	if len(searchResp.Games) == 0 {
		return nil, fmt.Errorf("❌ 未找到 AppID: %s", appID)
	}

	game := &searchResp.Games[0]
	fmt.Printf("🎮 %s\n", game.Name)

	return game, nil
}

// 准备下载
func (md *ManifestDownloader) prepareDownload(appID string) (*PrepareDownloadResponse, error) {
	baseURL := "https://manifest.morrenus.xyz"
	prepareURL := fmt.Sprintf("%s/download/prepare/%s", baseURL, appID)

	req, err := http.NewRequest("POST", prepareURL, strings.NewReader("{}"))
	if err != nil {
		return nil, fmt.Errorf("❌ 创建请求失败: %v", err)
	}

	// 设置请求头
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "*/*")
	req.Header.Set("User-Agent", md.config.UserAgent)
	req.Header.Set("Accept-Language", "zh-CN,zh-Hans;q=0.9")
	req.Header.Set("Accept-Encoding", "gzip, deflate")
	req.Header.Set("Origin", baseURL)
	req.Header.Set("Referer", baseURL+"/")
	req.Header.Set("Content-Length", "2")

	if md.config.ManifestCookie != "" {
		req.Header.Set("Cookie", strings.TrimSpace(md.config.ManifestCookie))
	}

	resp, err := md.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("❌ 请求失败: %v", err)
	}
	defer resp.Body.Close()

	body, err := md.readResponseBody(resp)
	if err != nil {
		return nil, fmt.Errorf("❌ 读取响应失败: %v", err)
	}

	// 检查是否是HTML错误页面
	bodyStr := string(body)
	if strings.Contains(bodyStr, "<!DOCTYPE html>") {
		return nil, fmt.Errorf("❌ 服务器返回HTML错误页面")
	}

	var prepareResp PrepareDownloadResponse
	if err := json.Unmarshal(body, &prepareResp); err != nil {
		fmt.Printf("❌ 服务器响应: %s\n", bodyStr)
		return nil, fmt.Errorf("❌ JSON解析失败: %v", err)
	}

	if !prepareResp.Success {
		return nil, fmt.Errorf("%s", prepareResp.Message)
	}

	if prepareResp.DownloadToken == "" {
		return nil, fmt.Errorf("❌ 未能获取下载令牌")
	}

	if prepareResp.Filename == "" {
		return nil, fmt.Errorf("❌ 未能获取文件名")
	}

	fmt.Printf("🔑 下载令牌: %s\n", prepareResp.DownloadToken)
	fmt.Printf("📊 剩余下载次数: %d\n", prepareResp.RemainingToday)

	return &prepareResp, nil
}

// 下载文件
func (md *ManifestDownloader) downloadManifestFile(appID, downloadToken, filename string, expectedSize int64) (string, error) {
	baseURL := "https://manifest.morrenus.xyz"
	downloadURL := fmt.Sprintf("%s/download/%s?token=%s", baseURL, appID, downloadToken)

	// 创建安全的文件名（移除特殊字符）
	safeFilename := regexp.MustCompile(`[^a-zA-Z0-9._-]`).ReplaceAllString(filename, "_")

	req, err := http.NewRequest("GET", downloadURL, nil)
	if err != nil {
		return "", fmt.Errorf("❌ 创建请求失败: %v", err)
	}

	// 设置请求头
	req.Header.Set("User-Agent", md.config.UserAgent)
	req.Header.Set("Referer", baseURL+"/")

	if md.config.ManifestCookie != "" {
		req.Header.Set("Cookie", strings.TrimSpace(md.config.ManifestCookie))
	}

	resp, err := md.client.Do(req)
	if err != nil {
		return "", fmt.Errorf("❌ 请求失败: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("❌ 下载失败，状态码: %d", resp.StatusCode)
	}

	// 创建文件
	file, err := os.Create(safeFilename)
	if err != nil {
		return "", fmt.Errorf("❌ 创建文件失败: %v", err)
	}
	defer file.Close()

	// 下载文件
	downloadedSize, err := io.Copy(file, resp.Body)
	if err != nil {
		return "", fmt.Errorf("❌ 下载文件失败: %v", err)
	}

	// 验证文件大小
	if expectedSize > 0 && downloadedSize != expectedSize {
		fmt.Printf("⚠️  文件大小不匹配！预期: %d bytes, 实际: %d bytes\n", expectedSize, downloadedSize)
	} else if expectedSize > 0 {
		fmt.Printf("✅ 文件大小验证通过\n")
	}

	fmt.Printf("💾 文件: %s\n", safeFilename)
	fmt.Printf("📏 大小: %d bytes (%.2f MB)\n", downloadedSize, float64(downloadedSize)/1024/1024)

	// 解压到ManifestHub目录
	if err := md.extractZipToManifestDir(safeFilename, appID); err != nil {
		return safeFilename, fmt.Errorf("❌ 解压文件失败: %v", err)
	}

	fmt.Printf("✅ 文件已解压到 %s/%s\n", md.baseDir, appID)
	return safeFilename, nil
}

// 使用Manifest API下载清单
func (md *ManifestDownloader) downloadWithManifestAPI(appID string) (bool, error) {
	// 检查是否配置了Cookie
	if md.config.ManifestCookie == "" {
		return false, nil // 没有配置cookie，返回false表示未尝试
	}

	fmt.Printf("🔍 使用Manifest API下载 AppID: %s\n", appID)

	// 搜索游戏信息
	_, err := md.searchGame(appID)
	if err != nil {
		return false, err
	}

	// 准备下载
	prepareResp, err := md.prepareDownload(appID)
	if err != nil {
		return false, err
	}

	// 下载文件
	zipFilename, err := md.downloadManifestFile(appID, prepareResp.DownloadToken, prepareResp.Filename, prepareResp.FileSize)
	if err != nil {
		return false, err
	}

	// 使用defer确保ZIP文件最终被删除
	defer func() {
		if err := os.Remove(zipFilename); err != nil {
			fmt.Printf("⚠️  删除原始文件失败: %v\n", err)
		}
	}()

	// 检查解压后的目录是否包含密钥文件
	appDir := filepath.Join(md.baseDir, appID)
	if !md.hasKeyFiles(appDir) {
		return false, fmt.Errorf("❌ 解压后的目录中未找到密钥文件")
	}

	if err := md.processDepotKeys(appID); err != nil {
		return false, fmt.Errorf("❌ 处理密钥文件失败: %v", err)
	}

	return true, nil
}
