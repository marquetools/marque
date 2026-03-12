<?xml version="1.0" encoding="utf-8"?>
<!-- **************************************************************** -->
<!--                            UNCLASSIFIED                          -->
<!-- **************************************************************** -->
<!-- ****************************************************************
  INTELLIGENCE COMMUNITY TECHNICAL SPECIFICATION  
  XML DATA ENCODING SPECIFICATION FOR 
  INFORMATION SECURITY MARKING METADATA (ISM.XML)
  ****************************************************************
  Module:   IC-ISM-PortionMark.xsl
  Creators: Office of the Director of National Intelligence
  Intelligence Community Chief Information Officer
  **************************************************************** -->
<!-- **************************************************************** -->
<!--                            INTRODUCTION                          -->
<!-- **************************************************************** -->
<!-- This XSLT file is one component of the ISM.XML Data Encoding 
  Specification (DES). Please see the document titled 
  XML DATA ENCODING SPECIFICATION FOR INFORMATION SECURITY MARKING METADATA 
  for a complete description of the encoding as well as list
  of all components.  
  
  It is envisioned that this XSLT or its components, as well as other
  parts of the DES may be overridden for localized implementations. 
  Therefore, permission to use, copy, modify and distribute this XSLT
  and the other parts of the DES for any purpose is hereby 
  granted in perpetuity.
  
  Please reference the preceding two paragraphs in all copies or
  variations. The developers make no representation about the
  suitability of the schema or DES for any purpose. It is provided 
  "as is" without expressed or implied warranty.
  
  If you modify this XSLT in any way label it
  as a variant of ISM.XML. 
  
  Please direct all questions, bug reports,or suggestions for changes
  to the points of contact identified in the document referenced above.
-->
<!-- **************************************************************** -->
<!--                            DESCRIPTION                           -->
<!--                                                                  -->
<!-- This XSLT 2 stylesheet renders a portion marking from the        -->
<!-- ISM attribute values of a portion-level element.  The rendered   -->
<!-- marking is compliant with the IC Register and Manual guidelines  -->
<!-- as of the 2019-AUG release of the manual                         -->
<!-- **************************************************************** -->


<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" version="2.0"
  xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:ism="urn:us:gov:ic:ism"
  xmlns:ism-func="urn:us:gov:ic:ism:functions">

  <xsl:import href="IC-ISM-Functions.xsl"/>

  <xsl:output method="text" encoding="UTF-8" media-type="text-plain" indent="no"/>
  <!-- If including this xsl causes "Content is not allowed in prolog" the importing 
  XSL is likely missing an output declaration -->

  <xsl:param name="warn-missing-classif" select="'MISSING CLASSIFICATION MARKING'"/>
  <xsl:param name="warn-parse-classif" select="'UNABLE TO DETERMINE CLASSIFICATION MARKING'"/>
  <xsl:param name="warn-parse-ownerproducer"
    select="concat($warn-parse-classif, ' - MISSING OWNER/PRODUCER')"/>
  <xsl:param name="warn-parse-relto" select="'UNABLE TO DETERMINE RELEASABILITY'"/>
  <xsl:param name="warn-parse-displayonly" select="'UNABLE TO DETERMINE DISPLAY ONLY'"/>
  <xsl:param name="warn-parse-eyes" select="'UNABLE TO DETERMINE EYES ONLY MARKINGS'"/>

  <xsl:param name="CUIRenderingRuleSet" select="''"/>
  <xsl:param name="SAPRenderingRuleSet" select="''"/>

  <!--***********************************************-->
  <!-- Mode for generating the CAPCO portion mark-->
  <!--***********************************************-->
  <xsl:template match="*[@ism:*]" mode="ism:portionmark">
    <xsl:param name="overalldissem"/>
    <xsl:param name="overallreleaseto"/>
    <xsl:call-template name="get.portionmark">
      <xsl:with-param name="overalldissem" select="$overalldissem"/>
      <xsl:with-param name="overallreleaseto" select="$overallreleaseto"/>
    </xsl:call-template>
  </xsl:template>

  <!-- **************************************************************** -->
  <!-- portionmark - renders the security portion marking for each of   -->
  <!--               the document's portion level elements.             -->
  <!-- **************************************************************** -->
  <!-- **************************************************************** -->
  <!-- NOTE: The "overalldissem" and "overallreleaseto" parameters are  -->
  <!--       used to compare the document-level "REL TO" or "EYES ONLY" -->
  <!--       dissemination controls to the corresponding portion-level  -->
  <!--       dissemination controls (as specified in the "dissem" and   -->
  <!--       "releaseto" parameters).                                   -->
  <!--                                                                  -->
  <!--       As per IC guidelines, "REL TO" and "EYES ONLY" portion     -->
  <!--       markings can be abbreviated when they would otherwise be   -->
  <!--       identical to the corresponding document-level markings.    -->
  <!--                                                                  -->
  <!--       The "overalldissem" and "overallreleaseto" parameters are  -->
  <!--       not required.  However, if the parameters are not passed   -->
  <!--       into the template, a comparison can not be made, in which  -->
  <!--       case the full "REL TO" or "EYES ONLY" dissemination        -->
  <!--       control markings will be rendered for the portion even     -->
  <!--       when the portion-level and document-level dissemination    -->
  <!--       control markings are the same.                             -->
  <!-- **************************************************************** -->
  <xsl:template name="portionmark">
    <xsl:param name="class"/>
    <xsl:param name="ownerproducer"/>
    <xsl:param name="joint"/>
    <xsl:param name="sci"/>
    <xsl:param name="sar"/>
    <xsl:param name="atomicenergymarkings"/>
    <xsl:param name="fgiopen"/>
    <xsl:param name="fgiprotect"/>
    <xsl:param name="dissem"/>
    <xsl:param name="releaseto"/>
    <xsl:param name="displayonly"/>
    <xsl:param name="cuiBasic"/>
    <xsl:param name="cuiSpecified"/>
    <xsl:param name="nonic"/>
    <xsl:param name="nonuscontrols"/>
    <xsl:param name="compliesWith"/>
    <xsl:param name="overalldissem"/>
    <xsl:param name="overallreleaseto"/>

    <!-- **** Normalize all of the parameters. **** -->
    <xsl:variable name="n-class" select="normalize-space($class)"/>
    <xsl:variable name="n-joint" select="normalize-space($joint)"/>

    <!-- Sort ownerproducer Based on CVE -->
    <xsl:variable name="n-ownerproducer">
      <xsl:value-of select="ism-func:sortOwnerProducer($ownerproducer)"/>
    </xsl:variable>


    <!-- Sort SCI alphabetically -->
    <xsl:variable name="n-sci">
      <xsl:value-of select="ism-func:sortSciControls($sci)"/>
    </xsl:variable>

    <!-- Sort atomicenergymarkings Based on CVE -->
    <!-- Requires 2020-JUN or later CVE with regex replaced by actual values.  -->
    <xsl:variable name="n-atomicenergymarkings">
      <xsl:value-of select="ism-func:sortAtomicenergymarkings($atomicenergymarkings)"/>
    </xsl:variable>

    <!-- Sort fgiopen Based on CVE -->
    <xsl:variable name="n-fgiopen">
      <xsl:value-of select="ism-func:sortFGIOpen($fgiopen)"/>
    </xsl:variable>

    <!-- Sort fgiprotect Based on CVE -->
    <!-- Should not matter since any protected renders as just FGI. -->
    <xsl:variable name="n-fgiprotect">
      <xsl:value-of select="ism-func:sortFGIProtected($fgiprotect)"/>
    </xsl:variable>

    <!-- **** Determine the set of dissemination controls that are not CUI limited dissem controls -->
    <xsl:variable name="dissemsNotCui">
      <xsl:if test="$dissem != ''">
        <xsl:value-of select="ism-func:get.dissemNotCUI($dissem)"/>
      </xsl:if>
    </xsl:variable>

    <!-- Variable to determine if there are any IC Register-specific control markings that are not CUI limited dissem controls -->
    <xsl:variable name="CUIandICcontrolMarkings">
      <xsl:choose>
        <xsl:when
          test="
            ($cuiBasic != '' or $cuiSpecified != '') and ($n-class = '' or $n-class = 'U') and
            (string($sci) = '' and string($sar) = '' and string($atomicenergymarkings) = '' and
            string($fgiopen) = '' and string($fgiprotect) = '' and string($nonic) = '' and string($dissemsNotCui) = '')">
          <xsl:value-of select="false()"/>
        </xsl:when>
        <xsl:otherwise>
          <xsl:value-of select="true()"/>
        </xsl:otherwise>
      </xsl:choose>
    </xsl:variable>

    <!-- Sort Dissem Based on CVE -->
    <xsl:variable name="n-dissem">
      <xsl:variable name="sortedDissem"
        select="ism-func:sortDissemControls($dissem, $compliesWith, $CUIandICcontrolMarkings)"/>
      <xsl:value-of select="replace(normalize-space($sortedDissem), 'OC OC-USGOV', 'OC-USGOV')"/>
    </xsl:variable>

    <!-- Sort RelTo Based on CVE -->
    <xsl:variable name="n-releaseto">
      <xsl:value-of select="ism-func:sortReleaseto($releaseto)"/>
    </xsl:variable>

    <!-- Sort DisplayOnly Based on CVE -->
    <xsl:variable name="n-displayonly">
      <xsl:value-of select="ism-func:sortDisplayonly($displayonly)"/>
    </xsl:variable>

    <!-- Sort NonIC Based on CVE -->
    <xsl:variable name="n-nonic">
      <xsl:value-of select="ism-func:sortNonic($nonic)"/>
    </xsl:variable>

    <!-- Sort cuiBasic alphabetically -->
    <xsl:variable name="n-cuiBasic">
      <xsl:value-of select="ism-func:sortCuiBasic($cuiBasic)"/>
    </xsl:variable>

    <!-- Sort cuiSpecified alphabetically -->
    <xsl:variable name="n-cuiSpecified">
      <xsl:value-of select="ism-func:sortCuiSpecified($cuiSpecified)"/>
    </xsl:variable>

    <xsl:variable name="n-nonuscontrls" select="normalize-space($nonuscontrols)"/>
    <xsl:variable name="n-overalldissem" select="normalize-space($overalldissem)"/>

    <!-- Sort overallreleaseto Based on CVE -->
    <xsl:variable name="n-overallreleaseto">
      <xsl:value-of select="ism-func:sortReleaseto($overallreleaseto)"/>
    </xsl:variable>

    <!-- **** Determine the classification marking **** -->
    <xsl:variable name="classVal">
      <xsl:choose>
        <xsl:when test="$n-class != ''">
          <xsl:choose>
            <!-- Multiple Owner Producers JOINT true -->
            <xsl:when test="$n-ownerproducer = ''">
              <xsl:value-of select="$warn-parse-ownerproducer"/>
            </xsl:when>
            <xsl:when
              test="contains($n-ownerproducer, ' ') and ($n-fgiprotect = '') and $joint = 'true'">
              <xsl:choose>
                <xsl:when test="not($n-class = ('TS', 'S', 'C', 'R', 'U'))">
                  <xsl:value-of select="$warn-parse-classif"/>
                </xsl:when>
                <xsl:otherwise>
                  <xsl:text>//JOINT </xsl:text>
                  <xsl:value-of select="$n-class"/>
                  <xsl:text> </xsl:text>
                  <xsl:value-of select="$n-ownerproducer"/>
                </xsl:otherwise>
              </xsl:choose>
            </xsl:when>

            <!-- Multiple Owner Producers JOINT false USA not one of the producers -->
            <xsl:when
              test="contains($n-ownerproducer, ' ') and not(contains($n-ownerproducer, 'USA')) and ($n-fgiprotect = '') and $joint != 'true'">
              <xsl:choose>
                <xsl:when test="not($n-class = ('TS', 'S', 'C', 'R', 'U'))">
                  <xsl:value-of select="$warn-parse-classif"/>
                </xsl:when>
                <xsl:otherwise>
                  <xsl:text>//</xsl:text>
                  <xsl:value-of select="$n-ownerproducer"/>
                  <xsl:text> </xsl:text>
                  <xsl:value-of select="$n-class"/>
                </xsl:otherwise>
              </xsl:choose>
            </xsl:when>

            <!-- Multiple Owner Producers JOINT false USA one of the producers invalid state-->
            <xsl:when
              test="contains($n-ownerproducer, ' ') and contains($n-ownerproducer, 'USA') and ($n-fgiprotect = '') and $joint != 'true'">
              <xsl:value-of select="$warn-parse-classif"/>
            </xsl:when>

            <xsl:when test="($n-ownerproducer = 'USA') and ($n-fgiopen != 'UNKNOWN')">
              <xsl:choose>
                <xsl:when test="not($n-class = ('TS', 'S', 'C', 'U'))">
                  <xsl:value-of select="$warn-parse-classif"/>
                </xsl:when>
                <xsl:otherwise>
                  <xsl:value-of select="$n-class"/>
                </xsl:otherwise>
              </xsl:choose>
            </xsl:when>
            <xsl:when test="$n-ownerproducer = 'NATO'">
              <xsl:choose>
                <xsl:when test="$n-class = 'TS'">
                  <xsl:text>//CTS</xsl:text>
                </xsl:when>
                <xsl:when test="$n-class = ('S', 'C', 'R', 'U')">
                  <xsl:text>//N</xsl:text>
                  <xsl:value-of select="$n-class"/>
                </xsl:when>
                <xsl:otherwise>
                  <xsl:value-of select="$warn-parse-classif"/>
                </xsl:otherwise>
              </xsl:choose>
              <xsl:if test="$n-nonuscontrls">
                <xsl:text>//</xsl:text>
                <xsl:value-of select="translate($n-nonuscontrls, ' ', '/')"/>
              </xsl:if>
            </xsl:when>
            <xsl:when test="starts-with($n-ownerproducer, 'NATO:')">
              <xsl:variable name="natoNacString">
                <xsl:call-template name="ism:get.nato.nac.portion">
                  <xsl:with-param name="source" select="$n-ownerproducer"/>
                </xsl:call-template>
              </xsl:variable>
              <xsl:choose>
                <xsl:when test="$n-class = ('S', 'C', 'R', 'U')">
                  <xsl:value-of select="concat('//N', $natoNacString, $n-class)"/>
                </xsl:when>
                <xsl:otherwise>
                  <xsl:value-of select="$warn-parse-classif"/>
                </xsl:otherwise>
              </xsl:choose>
              <xsl:if test="$n-nonuscontrls">
                <xsl:text>//</xsl:text>
                <xsl:value-of select="translate($n-nonuscontrls, ' ', '/')"/>
              </xsl:if>
            </xsl:when>

            <xsl:otherwise>
              <xsl:choose>
                <xsl:when test="not($n-class = ('TS', 'S', 'C', 'R', 'U'))">
                  <xsl:value-of select="$warn-parse-classif"/>
                </xsl:when>
                <xsl:otherwise>
                  <xsl:text>//</xsl:text>
                  <xsl:choose>
                    <xsl:when test="($n-fgiprotect != '') or ($n-fgiopen = 'UNKNOWN')">
                      <xsl:text>FGI</xsl:text>
                    </xsl:when>
                    <xsl:otherwise>
                      <xsl:value-of select="$n-ownerproducer"/>
                    </xsl:otherwise>
                  </xsl:choose>
                  <xsl:text> </xsl:text>
                  <xsl:value-of select="$n-class"/>
                </xsl:otherwise>
              </xsl:choose>
            </xsl:otherwise>
          </xsl:choose>
        </xsl:when>
        <xsl:otherwise>
          <xsl:value-of select="$warn-missing-classif"/>
        </xsl:otherwise>
      </xsl:choose>
    </xsl:variable>

    <!-- **** Determine the SCI marking **** -->
    <xsl:variable name="sciVal">
      <xsl:value-of select="ism-func:sciVal($n-sci, $n-nonuscontrls)"/>
    </xsl:variable>

    <!-- **** Determine AtomicEnergyMarking ****-->
    <xsl:variable name="atomicEnergyVal">
      <xsl:value-of select="ism-func:AEAVal($n-atomicenergymarkings, $n-nonuscontrls, false())"/>
    </xsl:variable>

    <!-- **** Determine the SAR marking **** -->
    <xsl:variable name="sarVal">
      <xsl:if test="$sar != ''">
        <xsl:text>//SAR-</xsl:text>
        <xsl:call-template name="ism:get.sar.pm">
          <xsl:with-param name="all" select="$sar"/>
        </xsl:call-template>
      </xsl:if>
    </xsl:variable>

    <!-- **** Determine the dissemination marking **** -->
    <xsl:variable name="dissemVal">
      <xsl:if test="$n-dissem != ''">
        <xsl:variable name="val" select="$n-dissem"/>
        <xsl:text>//</xsl:text>
        <xsl:call-template name="ism:get.dissem.pm">
          <xsl:with-param name="all" select="$val"/>
          <xsl:with-param name="relto" select="$n-releaseto"/>
          <xsl:with-param name="displayonly" select="$n-displayonly"/>
          <xsl:with-param name="overalldissem" select="$n-overalldissem"/>
          <xsl:with-param name="overallrelto" select="$n-overallreleaseto"/>
        </xsl:call-template>
      </xsl:if>
    </xsl:variable>

    <!-- **** Determine the non-IC marking **** -->
    <xsl:variable name="nonicVal">
      <xsl:if test="$n-nonic != ''">
        <xsl:variable name="val" select="$n-nonic"/>
        <xsl:variable name="DissemLookup" select="()"/>
        <xsl:text>//</xsl:text>
        <xsl:value-of select="ism-func:get.nonic($val, $DissemLookup)"/>
      </xsl:if>
    </xsl:variable>


    <!-- **** Determine the FGI marking **** -->
    <xsl:variable name="fgiVal">
      <!-- ******************************************************************************************************* -->
      <!-- FGI markings are only used when foreign government information is included in a US controlled document, -->
      <!-- or when the document is jointly controlled and 'USA' is an owner/producer and a non-US owner/producer   -->
      <!-- is protected.                                                                                           -->
      <!-- ******************************************************************************************************* -->
      <xsl:if
        test="(($n-ownerproducer = 'USA') or (contains($n-ownerproducer, 'USA') and $n-fgiprotect != ''))">
        <xsl:choose>
          <xsl:when
            test="(($n-fgiopen != '') and (not(contains($n-fgiopen, 'UNKNOWN'))) and ($n-fgiprotect = ''))">

            <xsl:text>//FGI </xsl:text>
            <xsl:value-of select="translate($n-fgiopen, '_:', '  ')"/>
            <xsl:if test="$n-nonuscontrls">
              <xsl:variable name="nonatocontrls">
                <xsl:value-of
                  select="
                    translate(
                    normalize-space(translate(translate(translate($n-nonuscontrls, 'BALK', ' '), 'BOHEMIA', ' '), 'ATOMAL', ' ')),
                    ' ', '/')"
                />
              </xsl:variable>
              <xsl:if test="$nonatocontrls">
                <xsl:value-of select="$nonatocontrls"/>
              </xsl:if>
            </xsl:if>
          </xsl:when>
          <xsl:when test="(($n-fgiprotect != '') or (contains($n-fgiopen, 'UNKNOWN')))">
            <!-- *************************************************************** -->
            <!-- Display the generic FGI marking when the document:              -->
            <!--                                                                 -->
            <!--   1.  contains some FGI from a protected source(s)              -->
            <!--   2.  contains some FGI from an unknown source(s)               -->
            <!--                                                                 -->
            <!-- *************************************************************** -->
            <xsl:text>//FGI</xsl:text>
          </xsl:when>
        </xsl:choose>
      </xsl:if>
    </xsl:variable>


    <!-- **** Determine the cuiBasic marking **** -->
    <xsl:variable name="cuiBasicVal">
      <xsl:if test="$n-cuiBasic != ''">
        <xsl:choose>
          <xsl:when test="$n-cuiSpecified != ''">
            <xsl:text>/</xsl:text>
          </xsl:when>
          <xsl:otherwise>
            <xsl:text>//</xsl:text>
          </xsl:otherwise>
        </xsl:choose>
        <xsl:value-of select="ism-func:get.cuiBasic($n-cuiBasic)"/>
      </xsl:if>
    </xsl:variable>

    <!-- **** Determine the cuiSpecified marking **** -->
    <xsl:variable name="cuiSpecifiedVal">
      <xsl:if test="$n-cuiSpecified != ''">
        <xsl:text>//</xsl:text>
        <xsl:value-of select="ism-func:get.cuiSpecified($n-cuiSpecified)"/>
      </xsl:if>
    </xsl:variable>

    <!-- **** Output the values as a single string **** -->
    <xsl:choose>
      <xsl:when
        test="
          ($cuiBasicVal != '' or $cuiSpecified != '') and ($n-class = '' or $n-class = 'U') and
          ($sciVal = '' and $sarVal = '' and $atomicEnergyVal = '' and $fgiVal = '' and $nonicVal = '' and $dissemsNotCui = '')">
        <xsl:text>CUI</xsl:text>
      </xsl:when>
      <xsl:otherwise>
        <xsl:value-of select="$classVal"/>
      </xsl:otherwise>
    </xsl:choose>
    <xsl:value-of select="$sciVal"/>
    <xsl:value-of select="$sarVal"/>
    <xsl:value-of select="$atomicEnergyVal"/>
    <xsl:value-of select="$fgiVal"/>
    <xsl:if
      test="
        ($cuiBasicVal != '' or $cuiSpecified != '') and
        (($sciVal != '' or $sarVal != '' or $atomicEnergyVal != '' or $fgiVal != '' or $dissemsNotCui != '')
        or ($n-class != '' and $n-class != 'U'))">
      <!-- or (($n-class = '' or $n-class = 'U') and $CUIRenderingRuleSet = 'DOD'))"> -->
      <xsl:text>//CUI</xsl:text>
    </xsl:if>
    <xsl:value-of select="$cuiSpecifiedVal"/>
    <xsl:value-of select="$cuiBasicVal"/>
    <xsl:value-of select="$dissemVal"/>
    <xsl:value-of select="$nonicVal"/>

  </xsl:template>



  <!-- ******************************************************************* -->
  <!-- A  routine for processing disseminationControl name tokens -->
  <!-- ******************************************************************* -->
  <xsl:template name="ism:get.dissem.pm">
    <xsl:param name="all"/>
    <xsl:param name="relto"/>
    <xsl:param name="displayonly"/>
    <xsl:param name="overalldissem"/>
    <xsl:param name="overallrelto"/>

    <xsl:for-each select="tokenize($all, ' ')">
      <!-- The dissemination control EXEMPT_FROM_ICD501_DISCOVERY is not rendered -->
      <xsl:if test="not(current() = 'EXEMPT_FROM_ICD501_DISCOVERY')">
        <!-- Add a preceding / for all but the first dissem control. -->
        <xsl:if test="position() != 1">
          <xsl:text>/</xsl:text>
        </xsl:if>
        <xsl:call-template name="ism:get.dissem.names">
          <xsl:with-param name="name" select="current()"/>
          <xsl:with-param name="rel" select="$relto"/>
          <xsl:with-param name="displayonly" select="$displayonly"/>
          <xsl:with-param name="overalldissem" select="$overalldissem"/>
          <xsl:with-param name="overallrelto" select="$overallrelto"/>
        </xsl:call-template>
      </xsl:if>
    </xsl:for-each>
  </xsl:template>

  <!-- *************************************************** -->
  <!-- Determine releasableTo name tokens for REL and EYES -->
  <!-- *************************************************** -->
  <xsl:template name="ism:get.dissem.names">
    <xsl:param name="name"/>
    <xsl:param name="rel"/>
    <xsl:param name="displayonly"/>
    <xsl:param name="overalldissem"/>
    <xsl:param name="overallrelto"/>

    <xsl:choose>
      <xsl:when test="$name = 'REL'">
        <xsl:choose>
          <xsl:when test="($rel != '')">
            <xsl:choose>
              <xsl:when test="(contains($overalldissem, 'REL') and ($overallrelto = $rel))">
                <xsl:text>REL</xsl:text>
              </xsl:when>
              <xsl:otherwise>
                <xsl:text>REL TO </xsl:text>
                <xsl:value-of select="ism-func:get.relOrDisplayString($rel)"/>
              </xsl:otherwise>
            </xsl:choose>
          </xsl:when>
          <xsl:otherwise>
            <xsl:value-of select="$warn-parse-relto"/>
          </xsl:otherwise>
        </xsl:choose>
      </xsl:when>
      <xsl:when test="$name = 'EYES'">
        <xsl:choose>
          <xsl:when test="($rel != '')">
            <xsl:choose>
              <xsl:when test="(contains($overalldissem, 'EYES') and ($overallrelto = $rel))">
                <xsl:text>EYES</xsl:text>
              </xsl:when>
              <xsl:otherwise>
                <xsl:value-of select="ism-func:get.eyesString($rel)"/>
                <xsl:text> EYES ONLY</xsl:text>
              </xsl:otherwise>
            </xsl:choose>
          </xsl:when>
          <xsl:otherwise>
            <xsl:value-of select="$warn-parse-eyes"/>
          </xsl:otherwise>
        </xsl:choose>
      </xsl:when>
      <xsl:when test="$name = 'DISPLAYONLY'">
        <xsl:text>DISPLAY ONLY </xsl:text>
        <xsl:choose>
          <xsl:when test="($displayonly != '')">
            <xsl:value-of select="ism-func:get.relOrDisplayString($displayonly)"/>
          </xsl:when>
          <xsl:otherwise>
            <xsl:value-of select="$warn-parse-displayonly"/>
          </xsl:otherwise>
        </xsl:choose>
      </xsl:when>
      <xsl:otherwise>
        <xsl:value-of select="$name"/>
      </xsl:otherwise>
    </xsl:choose>

  </xsl:template>

  <!-- ************************************************** -->
  <!-- A routine for processing SAR name tokens -->
  <!-- ************************************************** -->
  <xsl:template name="ism:get.sar.pm">
    <xsl:param name="all"/>

    <!-- Create tokenized SAR variable.                             -->
    <xsl:variable name="tokenizedSARinitial" select="tokenize($all, ' ')"/>
    <!-- We need to throw away the metadata for SAR owners and any required classification levels.  
         First throw away any classification levels.  Second, get the unique values.  Example if 
         a portion has DOD:TS:SAP1 and another portion has SAR-DOD:C:SAP1 then both will appear in the banner metadata
         (the ISM resource element).  We need to collapse down first to get SAR-DOD:SAP1 DOD:SAP1, then get the
         unique tokens which will be a single token SAR-DOD:SAP1.  Then throw away the owner DOD: because all
         we want to render is the SAP marking SAP1.  -->
    <!-- Create a STRING variable without any classification substrings -->
    <xsl:variable name="SARnoClassification">
      <xsl:for-each select="$tokenizedSARinitial">
        <xsl:if test="not(position() = 1)">
          <xsl:text> </xsl:text>
        </xsl:if>
        <xsl:choose>
          <!-- does token have two : characters.  If so, throw away the classification
            string, which is between the two : characters, and also throw away the SAR- prefix.  
            Otherwise, just take the entire token minus the SAR- prefix. Note will add back SAR- prefix
            as needed when doing the final rendering.  -->
          <xsl:when test="contains(substring-after(., ':'), ':')">
            <xsl:value-of
              select="concat(substring-before(substring-after(.,'SAR-'), ':'), ':', substring-after(substring-after(., ':'), ':'))"
            />
          </xsl:when>
          <xsl:otherwise>
            <xsl:value-of select="substring-after(.,'SAR-')"/>
          </xsl:otherwise>
        </xsl:choose>
      </xsl:for-each>
    </xsl:variable>
    <!-- Get the unique values of the form SAROwner:SAPMarking -->
    <xsl:variable name="tokenizedSARwithOwner"
      select="distinct-values(tokenize($SARnoClassification))"/>
    
    <!-- Convert sequence to string for sorting -->
    <xsl:variable name="stringSARwithOwner">
      <xsl:for-each select="$tokenizedSARwithOwner">
        <xsl:if test="not(position()=1)">
          <xsl:text> </xsl:text>
        </xsl:if>
        <xsl:value-of select="."/>
      </xsl:for-each>
    </xsl:variable>
    
    <!-- Sort SAR without classifications alphabetically.          -->
    <!-- Note we cannot sort the SARs until we have eliminated any -->
    <!-- classification requirements in the marking                -->
    <xsl:variable name="n-sar">
      <xsl:value-of select="ism-func:sortSar($stringSARwithOwner)"/>
    </xsl:variable>
    
    <!-- Tokenize again -->
    <xsl:variable name="tokenizedSAR" select="tokenize($n-sar)"/>

    <!-- Loop over all the SAR tokens -->
    <xsl:for-each select="$tokenizedSAR">
      <xsl:variable name="tokenizedSARToken" select="tokenize(current(), '-')"/>
      <!-- In non-DoD SARs, a dash signifies a compartment and two dashes signify a subcompartment.
               In DOD SARs, there are no compartments or subcompartments so always set $compartmentLevelCount to zero -->
      <xsl:variable name="compartmentLevelCount">
        <xsl:choose>
          <xsl:when test="substring-before(.,':')='DOD'">
            <xsl:value-of select="0"/>
          </xsl:when>
          <xsl:otherwise>
            <xsl:value-of select="count($tokenizedSARToken) - 1"/>
          </xsl:otherwise>
        </xsl:choose>
      </xsl:variable> 
      <!--  Now get an appropriate separator (dash, slash or space) to go before the SAP marking value -->
      <xsl:choose>
        <!-- Not the first SAR and has no compartment/subcompartments add a / -->
        <xsl:when test="$compartmentLevelCount = 0 and not(position() = 1)">
          <xsl:choose>
            <xsl:when test="$SAPRenderingRuleSet = 'DOD'">
              <xsl:text>/SAR-</xsl:text>
            </xsl:when>
            <xsl:otherwise>
              <xsl:text>/</xsl:text>
            </xsl:otherwise>
          </xsl:choose>
        </xsl:when>
        <!-- A compartment add a - -->
        <xsl:when test="$compartmentLevelCount = 1">
          <xsl:text>-</xsl:text>
        </xsl:when>
        <!-- A subcompartment add a space -->
        <xsl:when test="$compartmentLevelCount = 2">
          <xsl:text> </xsl:text>
        </xsl:when>
      </xsl:choose>
      <!-- Now generate the rendered value.  If no compartments/subcompartments, send the current $tokenizedSAR 
               token to ism-func:get.sar.name.  If there are compartments/subcompartments, then we need to send 
               the last part of the marking after the last dash, which is $tokenizedSARToken[last()], but we need to add back 
               the SAR owner in front of the last marking -->
      <xsl:choose>
        <xsl:when test="$compartmentLevelCount = 0">
          <xsl:value-of select="ism-func:get.sar.name(.)"/>
        </xsl:when>
        <xsl:otherwise>
          <!-- For compartments and subcompartments, we need to add back the SAR owner followed by colon,
                just before the SAR marking value -->
          <xsl:variable name="SARstringWithOwner" select="concat(substring-before(.,':'),':',$tokenizedSARToken[last()])"/>
          <xsl:value-of select="ism-func:get.sar.name($SARstringWithOwner)"/>
        </xsl:otherwise>
      </xsl:choose>
    </xsl:for-each>
  </xsl:template>



  <!-- ************************************************** -->
  <!-- A routine for processing atomicEnergyMarking tokens -->
  <!-- ************************************************** -->
  <xsl:template name="ism:get.atomicEnergyMarking.pm">
    <xsl:param name="all"/>
    <xsl:value-of select="ism-func:getAEA($all, false())"/>
  </xsl:template>

  <!-- ********************************************************** -->
  <!-- A generic template for getting a complete portion marking  -->
  <!--                                                            -->
  <!-- This template can be called without any parameters from    -->
  <!-- any stylesheet when the element for which a portion        -->
  <!-- marking is required is the current node.  When this        -->
  <!-- template is called, the output will include parentheses    -->
  <!-- and a space after the portion marking.                     -->
  <!-- ********************************************************** -->

  <!-- **************************************************************** -->
  <!-- NOTE: The "overalldissem" and "overallreleaseto" parameters are  -->
  <!--       used to compare the document-level "REL TO" or "EYES ONLY" -->
  <!--       dissemination controls to the corresponding portion-level  -->
  <!--       dissemination controls (as specified in the "dissem" and   -->
  <!--       "releaseto" parameters).                                   -->
  <!--                                                                  -->
  <!--       As per IC guidelines, "REL TO" and "EYES ONLY" portion     -->
  <!--       markings can be abbreviated when they would otherwise be   -->
  <!--       identical to the corresponding document-level markings.    -->
  <!--                                                                                                                          -->
  <!--       The "overalldissem" and "overallreleaseto" parameters are  -->
  <!--       not required.  However, if the parameters are not passed   -->
  <!--       into the template, a comparison can not be made, in which  -->
  <!--       case the full "REL TO" or "EYES ONLY" dissemination        -->
  <!--       control markings will be rendered for the portion even     -->
  <!--       when the portion-level and document-level dissemination    -->
  <!--       control markings are the same.                             -->
  <!-- **************************************************************** -->
  <xsl:template name="get.portionmark">
    <xsl:param name="overalldissem"/>
    <xsl:param name="overallreleaseto"/>

    <xsl:text>(</xsl:text>
    <xsl:call-template name="portionmark">
      <xsl:with-param name="class" select="./@ism:classification"/>
      <xsl:with-param name="ownerproducer" select="./@ism:ownerProducer"/>
      <xsl:with-param name="joint" select="./@ism:joint"/>
      <xsl:with-param name="sci" select="./@ism:SCIcontrols"/>
      <xsl:with-param name="sar" select="./@ism:SARIdentifier"/>
      <xsl:with-param name="atomicenergymarkings" select="./@ism:atomicEnergyMarkings"/>
      <xsl:with-param name="fgiopen" select="./@ism:FGIsourceOpen"/>
      <xsl:with-param name="fgiprotect" select="./@ism:FGIsourceProtected"/>
      <xsl:with-param name="dissem" select="./@ism:disseminationControls"/>
      <xsl:with-param name="releaseto" select="./@ism:releasableTo"/>
      <xsl:with-param name="displayonly" select="./@ism:displayOnlyTo"/>
      <xsl:with-param name="nonic" select="./@ism:nonICmarkings"/>
      <xsl:with-param name="cuiBasic" select="./@ism:cuiBasic"/>
      <xsl:with-param name="cuiSpecified" select="./@ism:cuiSpecified"/>
      <xsl:with-param name="nonuscontrols" select="./@ism:nonUSControls"/>
      <xsl:with-param name="compliesWith" select="./@ism:compliesWith"/>
      <xsl:with-param name="overalldissem" select="$overalldissem"/>
      <xsl:with-param name="overallreleaseto" select="$overallreleaseto"/>
    </xsl:call-template>
    <xsl:text>) </xsl:text>

  </xsl:template>

  <xsl:template name="get.portionmark.wxs">
    <xsl:param name="overalldissem"/>
    <xsl:param name="overallreleaseto"/>

    <xsl:call-template name="get.portionmark">
      <xsl:with-param name="overalldissem" select="$overalldissem"/>
      <xsl:with-param name="overallreleaseto" select="$overallreleaseto"/>
    </xsl:call-template>

  </xsl:template>

  <!-- ************************************************************ -->
  <!-- Get the NATO NAC string                                      -->
  <!-- ************************************************************ -->
  <xsl:template name="ism:get.nato.nac.portion">
    <xsl:param name="source"/>
    <xsl:value-of
      select="document('nacs.xml')//nacs/nac[@name = substring-after($source, ':')]/@portion"/>
  </xsl:template>


</xsl:stylesheet>
<!-- **************************************************************** -->
<!--                            UNCLASSIFIED                          -->
<!-- **************************************************************** -->
