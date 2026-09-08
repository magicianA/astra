# 天空数据来源与采集记录

采集日期：2026-09-04。所有天文下载均来自官方机构或官方数据中心。原始调研记录保留如下；现已生成运行时星表与完整 DE441 数据包，实际清单见 `data/catalog/manifest.json` 和 [实现记录](IMPLEMENTATION.md)。

## 实现阶段采集结果

Gaia DR3 的 `G ≤ 12` 异步查询返回 **3,087,828** 行、相同数量的唯一 source_id，与独立 COUNT 查询完全一致，低于 5,000,000 行上限。完整 ADQL 在 `scripts/fetch_gaia.py`；作业、原始 CSV 和行数记录在 `data/catalog/raw/`。Gaia 原始 CSV 包含十个天体测量相关系数，但当前运行时没有实现完整协方差传播。

合并后为 **3,087,396** 条：3,059,808 条 Gaia 数据和 27,588 条 Hipparcos-2 数据。官方交叉表缺失时，将 Hipparcos 自行传播到 2016.0，以 1 角秒内唯一候选匹配；3 角秒内有第二候选则拒绝。共补充 19,235 条位置匹配，单独保存匹配距离与质量标志。对匹配到 Hipparcos、但 Gaia RUWE 较差或缺自行的对象保留 Hipparcos；其他被可靠替换的 Hipparcos 条目不重复加入。BSC5 有 9,069 条匹配，用于亮星名称、光度和无附加异常标记的径向速度补充。

DE441 两个 SPK 均已完整下载，MD5 与 NAIF 的上游校验表一致；另外记录本地 SHA-256。IERS/USNO finals2000A、NAIF LSK 和 Noto 字体均已落盘。运行数据清单同时锁定星表、名称、历表、EOP、CIO 和时间公告，组成场景中的 data_id。

[IERS Bulletin C 72](https://datacenter.iers.org/data/latestVersion/bulletinC.txt)（2026-07-06）确认 2026 年末不插入闰秒，UTC−TAI 仍为 −37 秒。本地保存 `data/time/bulletin-c72.txt`，用于支持 2026 年内的 UTC 输入；未知更远未来使用 UT1/TT/TDB。

## 已取得的文件

2026-09-08 更新 [ESA/Gaia EDR3 全天颜色与亮度图](https://www.esa.int/ESA_Multimedia/Images/2020/12/The_colour_of_the_sky_from_Gaia_s_Early_Data_Release_32)。
改用官网 HI-RES 下载的原生 **4000×2000** 银道等距圆柱图，保存为 `data/background/gaia-edr3-hires-source.png`，
SHA-256 为 `68fef2ee355015f84a43105bbf296f5a8d8b38d2fe8fe1a068ba1402e9ce5550`。
以经度环绕的 3 像素中值滤波减弱单颗星点，保留原生尺寸，不做高斯模糊或放大插值。
渲染时在线性光空间构建 12 层 mipmap，三线性采样并修正经度接缝导数。
旧的 2000×1000 低分辨率来源仍留档；v2 运行纹理为 4000×2000，像素量为旧 1024×512 纹理的约 15.3 倍。

原图和衍生图均按 CC BY-SA 3.0 IGO 使用，署名 ESA/Gaia/DPAC，致谢 A. Moitinho；随应用附带授权链接与修改说明。
这是公开的颜色亮度可视化，不是经过绝对辐射校准的天空影像。处理参数、来源和衍生文件散列记录在
`data/background/manifest.json`，以 `gaia-edr3-diffuse-v2` 独立追踪。
v1 场景可继续读取其日期、地点和视角，重新渲染时使用 v2 显示效果，导出记录 v2；不保证重现 v1 的像素颜色。

| 本地文件 | 来源 | 本次核验 |
| --- | --- | --- |
| [hip2.dat.gz](research/hip2.dat.gz) | [CDS I/311 全主表](https://cdsarc.cds.unistra.fr/ftp/I/311/hip2.dat.gz) | 8,977,907字节；gzip正常；117,955行；117,955个唯一HIP |
| [hipparcos2-ReadMe.txt](research/hipparcos2-ReadMe.txt) | [CDS I/311 字段说明](https://cdsarc.cds.unistra.fr/ftp/I/311/ReadMe) | 说明包含历元、单位、定长字段与权矩阵定义 |
| [bsc5-catalog.gz](research/bsc5-catalog.gz) | [CDS V/50 主表](https://cdsarc.cds.unistra.fr/ftp/V/50/catalog.gz) | 573,921字节；gzip正常；9,110行 |
| [bright-star-ReadMe.txt](research/bright-star-ReadMe.txt) | [CDS V/50 字段说明](https://cdsarc.cds.unistra.fr/ftp/V/50/ReadMe) | 9,110条目录记录中9,096颗恒星，其他历史条目不能当作完整恒星 |
| [gaia-dr3-bright-sample.csv](research/gaia-dr3-bright-sample.csv) | [ESA Gaia TAP服务](https://gea.esac.esa.int/tap-server/tap/sync) | 200行数据，18列；参考历元2016.0；不是全天完整数据 |
| [de441-tech-comments.txt](research/de441-tech-comments.txt) | [JPL/NAIF DE441技术说明](https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/de441_tech-comments.txt) | 确认覆盖、解的来源和天体ID |
| [jpl-spk-summaries.txt](research/jpl-spk-summaries.txt) | [NAIF kernel摘要](https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/aa_summaries.txt) | 确认两个DE441分段与重叠、DE440/442短时段限制 |

精确字节数、SHA-256、原始链接、采集日期见 [下载清单](research/download-inventory.json)。SHA-256为本地计算，用于以后识别同一快照；没有把它称作已经与上游独立发布散列核对的结果。

## 星表检查发现

### Hipparcos-2

该文件使用ICRS方向，参考历元J1991.25。不是“所有数值均在J2000”，也不能只看表名就传播。[ESA 目录元数据说明](https://www.cosmos.esa.int/web/esdc/esasky-catalogues)

本次仅做数据结构与质量概况检查：

- 117,955条唯一HIP记录，与CDS说明一致。
- 4,013条视差≤0，需要距离未知或统计距离策略。
- 7,982条 `Hp≤6.5`；Hp不是Johnson V，不能直接称其为某一肉眼星等的严格完整数目。
- 11,236条 `Nc>1`，说明多分量信息必须保留。
- 主表没有径向速度；不能把它独自当成全部恒星的完整六维状态。
- `UW`为上三角权矩阵，不能直接当作协方差数组；按说明重建并检查正定性。

没有下载7参数、9参数、VIM等附表；只有主表。实现时若要使用那些特殊解，需要获取配套数据并明确有效期，不能外推观测窗口中的加速度多项式5000年。

### Bright Star Catalogue

该文件提供J2000/FK5相关方向、自行、V星等、颜色、光谱、部分日心径向速度及备注标记。主要用于亮星名字/光度与辅助补全，不作为Gaia质量的天体测量来源。

同一行可能缺字段，末尾空格可能被截掉。定长解析应允许按规定记录长度补齐尾部空格，仍需校验关键字段；不能要求每条原始物理行恰好197字节。当前没有取得 `notes` 备注正文，使用标记作高质量判断前须补取。

源版本是第5版预备版；数据说明明确记录了旧目录中保留编号的非恒星条目。统计时区分目录记录数、具有有效位置的对象数和实际恒星数。

### Gaia DR3样本

请求协议为TAP同步查询，`REQUEST=doQuery`、`LANG=ADQL`、`FORMAT=csv`，查询内容如下。该查询仅用于调研，是按source_id排序的非随机小样本，不可用于推算全表缺失率或天空均匀分布。

```sql
SELECT TOP 200
  source_id, ref_epoch, ra, dec, parallax, pmra, pmdec,
  radial_velocity, radial_velocity_error,
  phot_g_mean_mag, bp_rp, ruwe, astrometric_params_solved,
  ra_error, dec_error, parallax_error, pmra_error, pmdec_error
FROM gaiadr3.gaia_source
WHERE phot_g_mean_mag < 6
ORDER BY source_id
```

实际返回200条。51条缺径向速度，20条缺视差。样本没有完整的相关系数列，因此不足以进行正式协方差传播。实现阶段正式查询增加10个天体测量相关系数、光度误差/质量、双星/交叉匹配所需表，并保存完整ADQL。

正式包方案为按天区分批提取 `G≤12` 的候选、补充近邻/高自行对象、再与亮星表合并。使用TAP异步作业处理较大查询，检查服务的截断/overflow状态；保存每批查询、完成状态、行数和去重结果。不能把一个达到服务行数上限的响应当作完整全天表。

Gaia的数据字段定义及TCB历元、自行约定以 [DR3官方数据模型](https://gea.esac.esa.int/archive/documentation/GDR3/Gaia_archive/chap_datamodel/sec_dm_main_source_catalogue/ssec_dm_gaia_source.html) 为准。

## 实现阶段获取的资源

| 资源 | 官方入口 | 获取与使用约定 |
| --- | --- | --- |
| DE441第一部分 | [de441_part-1.bsp](https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/de441_part-1.bsp) | 約1.5GB级；覆盖目标区间的早段，必须取得 |
| DE441第二部分 | [de441_part-2.bsp](https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/de441_part-2.bsp) | 約1.5GB级；覆盖目标区间的晚段，必须取得 |
| 地球方向参数 | [IERS/USNO产品](https://maia.usno.navy.mil/) | 采用含UT1−UTC、极移、天极改正的明确格式快照，保留观测/预报标记 |
| 历史/近期ΔT | [USNO ΔT](https://maia.usno.navy.mil/products/deltaT) | 记录覆盖区间，范围外进入模型 |
| 长期ΔT模型 | [NASA多项式说明](https://eclipse.gsfc.nasa.gov/SEcat5/deltatpoly.html) | 作为近似/外推，不能宣称全部时间段已被历史观测约束 |
| SPICE闰秒kernel | [NAIF LSK目录](https://naif.jpl.nasa.gov/pub/naif/generic_kernels/lsk/) | 固定版本与有效信息，未知未来闰秒不假设已发生 |
| 天体形状/方向 | [NAIF PCK目录](https://naif.jpl.nasa.gov/pub/naif/generic_kernels/pck/) | 半径可作静态几何参数；方向/天平动须独立核验覆盖 |
| 现代行星中心/卫星状态 | [NAIF卫星SPK目录](https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/satellites/) | 按需启用，不能由DE441覆盖期推定卫星kernel也覆盖万年 |
| Gaia扩展目录 | [ESA数据访问](https://www.cosmos.esa.int/web/gaia/data-access) | 固定DR3版本，未来版本必须显式迁移 |

DE441原始说明中的边界是：第一部分JD −3100015.50至2440432.50，第二部分JD2440400.50至8000016.50。单位为历表时间的JD；日期文字可能采用不同历法，应以数值JD和SPK元数据为最终依据。导入测试必须查验每个body的中心链及光行时需要的边界余量。

## 署名与数据条款

- Gaia：保留ESA/Gaia/DPAC署名，按官方页面引用任务和DR3论文。[官方说明](https://gea.esac.esa.int/archive/documentation/GDR3/Miscellaneous/sec_credit_and_citation_instructions/)
- Hipparcos/Tycho：ESA目录页列出CC BY-NC 3.0 IGO和Credit: ESA。CDS提供的具体再处理产品在正式再分发前需核对其适用条款；当前只作为本地调研材料，不认定为可任意商业分发。[ESA来源](https://www.cosmos.esa.int/web/hipparcos/catalogues)
- BSC：保留Hoffleit/Warren和CDS原始说明；本次未确认可直接用于任何商业分发，分发版需完成该项核对。
- DE441/SPICE：保留JPL/NAIF来源、论文和工具使用条款；代码库许可证与科学数据/纹理条款分别处理。[NAIF规则](https://naif.jpl.nasa.gov/naif/rules.html)
- 图片/纹理：本次没有采集显示纹理；以后逐项记录授权，不将科学数据开放性自动套用到网站图片。

## 进一步的计算依据

| 主题 | 主来源 |
| --- | --- |
| DE441模型 | [JPL论文入口](https://ssd.jpl.nasa.gov/doc/de440_de441.html) |
| 标准时间/姿态/天体测量 | [SOFA C资料与cookbooks](https://www.iausofa.org/2023-10-11c) |
| 长期岁差 | [ERFA ltp及论文引用](https://raw.githubusercontent.com/liberfa/erfa/master/src/ltp.c) |
| 恒星空间运动 | [ERFA starpm](https://raw.githubusercontent.com/liberfa/erfa/master/src/starpm.c) |
| 地球参考系与非旋转原点 | [IERS Conventions第5章](https://iers-conventions.obspm.fr/content/chapter5/icc5.pdf) |
| 观测量验证 | [JPL Horizons手册](https://ssd.jpl.nasa.gov/horizons/manual.html) |

这些来源用于设计与实现。现代参考测试和万年区间数值检查见实现记录；数值收敛、区间可计算和现代单点比对不代表万年物理精度已经得到验证。
