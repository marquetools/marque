<?xml version="1.0" encoding="utf-8"?>
<!-- **************************************************************** -->
<!--                            UNCLASSIFIED                          -->
<!-- **************************************************************** -->
<!-- ****************************************************************
  INTELLIGENCE COMMUNITY TECHNICAL SPECIFICATION  
  XML DATA ENCODING SPECIFICATION FOR 
  INFORMATION SECURITY MARKING METADATA (ISM.XML)
  ******************************************************************* -->
<!-- Module:     IC-ISM-ClassDeclass.xsl                              -->
<!-- Date:     2011-08-12                                             -->
<!-- Creators: Office of the Director of National Intelligence
     Intelligence Community Chief Information Officer                 -->
<!-- **************************************************************** -->
<!-- **************************************************************** -->
<!--                            INTRODUCTION                          -->
<!--                                                                  -->
<!-- Intelligence Community Information Security Marking (IC ISM)     -->
<!-- standard was developed by the IC Security Panel for the express  -->
<!-- purpose of promoting CAPCO security marking interoperability     -->
<!-- between members of the Intelligence Community.                   -->
<!-- **************************************************************** -->
     
<!-- **************************************************************** -->
<!--                            DESCRIPTION                           -->
<!--                                                                  -->
<!-- This stylesheet outputs classification/declassification block    -->
<!-- content including the "Classified by", "Reason", "Derived from", -->
<!-- and/or "Declassify on" information required by the CAPCO         -->
<!-- Implementation Manual pursuant to ISOO Directive 1 and Executive -->
<!-- Order 13526.                                                     -->
<!-- **************************************************************** -->

<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" version="2.0"
  xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:ism="urn:us:gov:ic:ism"
  xmlns:ism-func="urn:us:gov:ic:ism:functions">

  <xsl:output method="text" encoding="UTF-8" media-type="text-plain" indent="no"/>
  <!-- If including this xsl causes "Content is not allowed in prolog" the importing 
  XSL is likely missing an output declaration -->

  <xsl:import href="IC-ISM-SecurityBanner.xsl"/>

  <xsl:param name="CUIRenderingRuleSet" select="''"/>

  <!--***********************************************-->
  <!-- Generate the Classification Authority Block for the current element-->
  <!--***********************************************-->
  <xsl:template match="*[@ism:*]" mode="ism:authority">
    <xsl:param name="delimiter"/>
    <xsl:call-template name="get.class.declass">
      <xsl:with-param name="delimiter" select="$delimiter"/>
    </xsl:call-template>
  </xsl:template>

  <!-- **************************************************************** -->
  <!-- get.class.declass - Calls template class.declass with parameters from the element's ISM attributes-->
  <!-- **************************************************************** -->
  <xsl:template name="get.class.declass">
    <xsl:param name="delimiter"/>

    <xsl:call-template name="class.declass">
      <xsl:with-param name="classification" select="@ism:classification"/>
      <xsl:with-param name="classifiedby" select="@ism:classifiedBy"/>
      <xsl:with-param name="derivativelyclassifiedby" select="@ism:derivativelyClassifiedBy"/>
      <xsl:with-param name="classificationreason" select="@ism:classificationReason"/>
      <xsl:with-param name="derivedfrom" select="@ism:derivedFrom"/>
      <xsl:with-param name="declassdate" select="@ism:declassDate"/>
      <xsl:with-param name="declassexception" select="@ism:declassException"/>
      <xsl:with-param name="declassevent" select="@ism:declassEvent"/>
      <xsl:with-param name="cuiControlledBy" select="@ism:cuiControlledBy"/>
      <xsl:with-param name="cuiDecontrolDate" select="@ism:cuiDecontrolDate"/>
      <xsl:with-param name="cuiDecontrolEvent" select="@ism:cuiDecontrolEvent"/>
      <xsl:with-param name="cuiControlledByOffice" select="@ism:cuiControlledByOffice"/>
      <xsl:with-param name="cuiPOC" select="@ism:cuiPOC"/>
      <xsl:with-param name="sci" select="./@ism:SCIcontrols"/>
      <xsl:with-param name="atomicenergymarkings" select="./@ism:atomicEnergyMarkings"/>
      <xsl:with-param name="sar" select="./@ism:SARIdentifier"/>
      <xsl:with-param name="fgiopen" select="./@ism:FGIsourceOpen"/>
      <xsl:with-param name="fgiprotect" select="./@ism:FGIsourceProtected"/>
      <xsl:with-param name="nonic" select="./@ism:nonICmarkings"/>
      <xsl:with-param name="dissem" select="./@ism:disseminationControls"/>
      <xsl:with-param name="releaseto" select="./@ism:releasableTo"/>
      <xsl:with-param name="displayonly" select="./@ism:displayOnlyTo"/>
      <xsl:with-param name="cuiBasic" select="./@ism:cuiBasic"/>
      <xsl:with-param name="cuiSpecified" select="./@ism:cuiSpecified"/>
      <xsl:with-param name="ownerproducer" select="./@ism:ownerProducer"/>
      <xsl:with-param name="overallCompliesWith" select="./@ism:compliesWith"/>
      <xsl:with-param name="delimiter" select="$delimiter"/>
    </xsl:call-template>

  </xsl:template>

  <!-- **************************************************************** -->
  <!-- class.declass - Determines the class/declass block content and   -->
  <!--                 calls a template to concatenate the content into -->
  <!--                 a delimited string                               -->
  <!-- **************************************************************** -->
  <xsl:template name="class.declass">
    <xsl:param name="classification"/>
    <xsl:param name="classifiedby"/>
    <xsl:param name="derivativelyclassifiedby"/>
    <xsl:param name="classificationreason"/>
    <xsl:param name="derivedfrom"/>
    <xsl:param name="declassdate"/>
    <xsl:param name="declassexception"/>
    <xsl:param name="declassevent"/>
    <xsl:param name="delimiter"/>
    <xsl:param name="cuiControlledBy"/>
    <xsl:param name="cuiDecontrolDate"/>
    <xsl:param name="cuiDecontrolEvent"/>
    <xsl:param name="cuiControlledByOffice"/>
    <xsl:param name="cuiPOC"/>
    <xsl:param name="cuiBasic"/>
    <xsl:param name="cuiSpecified"/>
    <xsl:param name="sci"/>
    <xsl:param name="atomicenergymarkings"/>
    <xsl:param name="sar"/>
    <xsl:param name="fgiopen"/>
    <xsl:param name="fgiprotect"/>
    <xsl:param name="nonic"/>
    <xsl:param name="dissem"/>
    <xsl:param name="releaseto"/>
    <xsl:param name="displayonly"/>
    <xsl:param name="ownerproducer"/>
    <xsl:param name="overallCompliesWith"/>
    <xsl:param name="compliesWith" select="./@ism:compliesWith"/>
    <!-- replace with overall complies with when build umbrella stylesheets -->

    <xsl:variable name="class-declass-delimiter">
      <xsl:choose>
        <xsl:when test="not($delimiter) or ($delimiter = '')">
          <xsl:text>|</xsl:text>
        </xsl:when>
        <xsl:otherwise>
          <xsl:value-of select="$delimiter"/>
        </xsl:otherwise>
      </xsl:choose>
    </xsl:variable>

    <xsl:variable name="n-classification" select="normalize-space($classification)"/>
    <xsl:variable name="n-classifiedby" select="normalize-space($classifiedby)"/>
    <xsl:variable name="n-derivativelyclassifiedby"
      select="normalize-space($derivativelyclassifiedby)"/>
    <xsl:variable name="n-classificationreason" select="normalize-space($classificationreason)"/>
    <xsl:variable name="n-derivedfrom" select="normalize-space($derivedfrom)"/>
    <xsl:variable name="n-declassdate" select="normalize-space($declassdate)"/>
    <xsl:variable name="n-declassexception" select="normalize-space($declassexception)"/>
    <xsl:variable name="n-declassevent" select="normalize-space($declassevent)"/>
    <xsl:variable name="n-cuiControlledBy" select="normalize-space($cuiControlledBy)"/>
    <xsl:variable name="n-cuiDecontrolDate" select="normalize-space($cuiDecontrolDate)"/>
    <xsl:variable name="n-cuiDecontrolEvent" select="normalize-space($cuiDecontrolEvent)"/>
    <xsl:variable name="n-overallCompliesWith" select="normalize-space($overallCompliesWith)"/>
    <xsl:variable name="n-compliesWith" select="normalize-space($compliesWith)"/>
    <xsl:variable name="n-cuiControlledByOffice" select="normalize-space($cuiControlledByOffice)"/>
    <xsl:variable name="n-cuiPOC" select="normalize-space($cuiPOC)"/>

    <!-- Sort ownerproducer Based on CVE -->
    <xsl:variable name="n-ownerproducer">
      <xsl:value-of select="ism-func:sortOwnerProducer($ownerproducer)"/>
    </xsl:variable>

    <!-- Sort cuiBasic alphabetically -->
    <xsl:variable name="n-cuiBasic">
      <xsl:value-of select="ism-func:sortCuiBasic($cuiBasic)"/>
    </xsl:variable>

    <!-- Sort cuiSpecified alphabetically -->
    <xsl:variable name="n-cuiSpecified">
      <xsl:value-of select="ism-func:sortCuiSpecified($cuiSpecified)"/>
    </xsl:variable>

    <!-- **** Determine the cuiSpecified marking **** -->
    <xsl:variable name="cuiSpecifiedVal">
      <xsl:if test="$n-cuiSpecified != ''">
        <xsl:value-of select="ism-func:get.cuiSpecified($n-cuiSpecified)"/>
      </xsl:if>
    </xsl:variable>

    <!-- **** Determine the set of dissemination controls that are CUI limited dissem controls -->
    <xsl:variable name="dissemCUI">
      <xsl:if test="$dissem != ''">
        <xsl:value-of select="ism-func:get.dissemCUI($dissem)"/>
      </xsl:if>
    </xsl:variable>
    
    <!-- **** Determine the set of dissemination controls that are NOT CUI limited dissem controls -->
    <xsl:variable name="dissemsNotCui">
      <xsl:if test="$dissem != ''">
        <xsl:value-of select="ism-func:get.dissemNotCUI($dissem)"/>
      </xsl:if>
    </xsl:variable>
    
    <!-- Variable to determine if there are any IC Register-specific control markings that are not CUI limited dissem controls -->
    <xsl:variable name="CUIandICcontrolMarkings">
      <xsl:choose>
        <xsl:when test="($cuiBasic != '' or $cuiSpecified != '') and ($n-classification = '' or $n-classification = 'U') and
          (string($sci) = '' and string($sar) = '' and string($atomicenergymarkings) = '' and 
          string($fgiopen) = '' and string($fgiprotect) = '' and string($nonic) = '' and string($dissemsNotCui) = '')">
          <xsl:value-of select="false()"/>
        </xsl:when>
        <xsl:otherwise>
          <xsl:value-of select="true()"/>
        </xsl:otherwise>
      </xsl:choose>
    </xsl:variable>
      
      <!-- Sort Dissem Based on CVE and on the value of the document's ism:compliesWith -->
      <xsl:variable name="n-dissem">
        <xsl:variable name="sortedDissem" select="ism-func:sortDissemControls($dissemCUI, $compliesWith, $CUIandICcontrolMarkings)"/>
        <xsl:value-of select="replace(normalize-space($sortedDissem), 'OC OC-USGOV', 'ORCON-USGOV')"/>
      </xsl:variable>

    <!-- Sort RelTo Based on CVE -->
    <xsl:variable name="n-releaseto">
      <xsl:value-of select="ism-func:sortReleaseto($releaseto)"/>
    </xsl:variable>

    <!-- Sort DisplayOnly Based on CVE -->
    <xsl:variable name="n-displayonly">
      <xsl:value-of select="ism-func:sortDisplayonly($displayonly)"/>
    </xsl:variable>

    <xsl:if
      test="
        ($n-classification and ($n-classification != 'U')) or
        ($n-classifiedby or $n-classificationreason or $n-derivedfrom or
        $n-declassdate or $n-declassexception or $n-declassevent)
        or ($n-compliesWith = 'USA-CUI-ONLY' or contains($n-compliesWith, 'USA-CUI'))">
      <xsl:variable name="warning-missing-classif">
        <xsl:if test="$n-classification = '' and not($n-compliesWith = 'USA-CUI-ONLY')">
          <xsl:text>(&#xA0;WARNING! This document does not contain a required overall classification marking.&#xA0;)</xsl:text>
        </xsl:if>
      </xsl:variable>
      <xsl:variable name="warning-unclass-and-classified-markings">
        <xsl:if
          test="
            ($n-classification = 'U' and ($n-classifiedby or $n-classificationreason or $n-derivedfrom or
            $n-declassdate or $n-declassexception or $n-declassevent))">
          <xsl:text>(&#xA0;WARNING! This document contains overall markings for both an unclassified and a classified document.&#xA0;)</xsl:text>
        </xsl:if>
      </xsl:variable>
      <xsl:variable name="warning-missing-classifiedBy">
        <xsl:if
          test="$n-classification and ($n-classification != 'U') and ($n-classifiedby = '') and ($n-derivedfrom = '')">
          <xsl:text>(&#xA0;WARNING! This document does not contain required markings for either an originally or derivatively classified document.&#xA0;)</xsl:text>
        </xsl:if>
      </xsl:variable>
      <xsl:variable name="classified-by-line" select="$n-classifiedby"/>
      <xsl:variable name="derivatively-classified-by-line" select="$n-derivativelyclassifiedby"/>
      <xsl:variable name="derived-from-line" select="$n-derivedfrom"/>
      <xsl:variable name="reason-line" select="$n-classificationreason"/>
      <xsl:variable name="warning-both-original-and-derivatively-classified">
        <xsl:if test="($n-classification != 'U') and $n-classifiedby and $n-derivedfrom">
          <xsl:text>(&#xA0;WARNING! This document contains markings for both an originally and derivatively classified document.&#xA0;)</xsl:text>
        </xsl:if>
      </xsl:variable>
      <xsl:variable name="warning-missing-classificationReason">
        <xsl:if
          test="$n-classification and ($n-classification != 'U') and $n-classifiedby and ($n-classificationreason = '')">
          <xsl:text>(&#xA0;WARNING! The reason for the classification decision should be specified for an originally classified document.&#xA0;)</xsl:text>
        </xsl:if>
      </xsl:variable>
      <xsl:variable name="warning-missing-declass-instructions">
        <xsl:if
          test="
            $n-classification and ($n-classification != 'U') and
            ($n-declassdate = '') and ($n-declassexception = '') and ($n-declassevent = '')">
          <xsl:text>(&#xA0;WARNING! This document does not contain required declassification instructions or markings.&#xA0;)</xsl:text>
        </xsl:if>
      </xsl:variable>
      <xsl:variable name="warning-missing-declass-info">
        <xsl:if
          test="
            $n-classification and ($n-classification != 'U')
            and $n-declassexception
            and not(contains($n-declassexception, 'AEA'))
            and not(contains($n-declassexception, 'NATO'))
            and not(contains($n-declassexception, 'NATO-AEA'))
            and not(contains($n-declassexception, '25X1-human'))
            and not(contains($n-declassexception, '50X1-HUM'))
            and not(contains($n-declassexception, '50X2-WMD'))
            and ($n-declassdate = '')
            and ($n-declassevent = '')">
          <xsl:text>(&#xA0;WARNING! A declassification date or event should be specified for a document with a 25X or 50X declassification exemption, unless the document has a declassification exemption of 25X1-human, 50X1-HUM, 50X2-WMD, AEA, or  NATO.&#xA0;)</xsl:text>
        </xsl:if>
      </xsl:variable>
      <xsl:variable name="declassify-on-line">
        <xsl:if test="$n-declassexception">
          <xsl:choose>
            <xsl:when test="$n-declassexception = 'AEA'">
              <xsl:text>Not Applicable to RD/FRD/TFNI portions. See source list for NSI portions.</xsl:text>
            </xsl:when>
            <xsl:when test="$n-declassexception = 'NATO'">
              <xsl:text>Not Applicable to NATO portions. See source list for NSI portions.</xsl:text>
            </xsl:when>
            <xsl:when test="$n-declassexception = 'NATO-AEA'">
              <xsl:text>Not Applicable to RD/FRD/TFNI and NATO portions. See source list for NSI portions.</xsl:text>
            </xsl:when>
            <xsl:otherwise>
              <xsl:value-of select="$n-declassexception"/>
            </xsl:otherwise>
          </xsl:choose>
        </xsl:if>
        <xsl:if test="$n-declassdate">
          <xsl:if test="$n-declassexception">
            <xsl:text>, </xsl:text>
          </xsl:if>
          <xsl:value-of select="format-date(xs:date($n-declassdate), '[MNn] [D], [Y0001]')"/>
        </xsl:if>
        <xsl:if test="$n-declassevent">
          <xsl:if test="$n-declassexception and ($n-declassdate = '')">
            <xsl:text>, </xsl:text>
          </xsl:if>
          <xsl:if test="$n-declassdate">
            <xsl:text> or </xsl:text>
          </xsl:if>
          <xsl:value-of select="$n-declassevent"/>
        </xsl:if>
      </xsl:variable>
      <xsl:variable name="warning-invalid-declass-date-and-exemption">
        <xsl:if test="$n-declassdate and (contains($n-declassexception, '25X1-human'))">
          <xsl:text>(&#xA0;WARNING! This document contains both a declassification date and a declassification exemption of 25X1-human.&#xA0;)</xsl:text>
        </xsl:if>
      </xsl:variable>
      <xsl:variable name="warning-invalid-declass-event-and-exemption">
        <xsl:if test="$n-declassevent and (contains($n-declassexception, '25X1-human'))">
          <xsl:text>(&#xA0;WARNING! This document contains both a declassification event and a declassification exemption of 25X1-human.&#xA0;)</xsl:text>
        </xsl:if>
      </xsl:variable>

      <!-- **************************************************************** -->
      <!-- Get data for CUI block if document contains CUI                  -->
      <!-- **************************************************************** -->

      <xsl:variable name="warning-missing-cuiControlledBy">
        <xsl:if test="$n-compliesWith = 'USA-CUI-ONLY' or contains($n-compliesWith, 'USA-CUI')">
          <xsl:if test="$n-cuiControlledBy = ''">
            <xsl:text>(&#xA0;WARNING! This document contains CUI markings but does not contain a required overall Controlled By: marking.&#xA0;)</xsl:text>
          </xsl:if>
        </xsl:if>
      </xsl:variable>
      <xsl:variable name="cui-controlled-by-line" select="$n-cuiControlledBy"/>

      <xsl:variable name="cui-decontrol-on-line">
        <xsl:if test="$n-cuiDecontrolDate">
          <xsl:value-of select="format-date(xs:date($n-cuiDecontrolDate), '[MNn] [D], [Y0001]')"/>
        </xsl:if>
        <xsl:if test="$n-cuiDecontrolEvent">
          <xsl:if test="$n-cuiDecontrolDate">
            <xsl:text> or </xsl:text>
          </xsl:if>
          <xsl:value-of select="$n-cuiDecontrolEvent"/>
        </xsl:if>
      </xsl:variable>

      <xsl:variable name="cui-controlled-by-office-line" select="$n-cuiControlledByOffice"/>
      <xsl:variable name="cui-POC-line" select="$n-cuiPOC"/>

      <xsl:variable name="cui-basic-line" select="$n-cuiBasic"/>
      <xsl:variable name="cui-specified-line" select="replace($cuiSpecifiedVal, '/', ' ')"/>
      
      <xsl:variable name="cui-categories-line">
        <xsl:choose>
          <xsl:when test="$cui-basic-line != '' and $cui-specified-line != ''">
            <xsl:value-of select="concat(string($cui-basic-line),' ',string($cui-specified-line))"/>
          </xsl:when>
          <xsl:otherwise>
            <xsl:value-of select="concat(string($cui-basic-line), string($cui-specified-line))"/>
          </xsl:otherwise>
        </xsl:choose>
      </xsl:variable>

      <!-- **** Determine the dissemination marking **** -->
      <xsl:variable name="dissemVal">
        <xsl:if test="$n-dissem != ''">
          <xsl:call-template name="ism:get.dissem.banner">
            <xsl:with-param name="all" select="$n-dissem"/>
            <xsl:with-param name="relto" select="$n-releaseto"/>
            <xsl:with-param name="displayonly" select="$n-displayonly"/>
            <xsl:with-param name="ownerproducer" select="$n-ownerproducer"/>
          </xsl:call-template>
        </xsl:if>
      </xsl:variable>

      <xsl:call-template name="ism:concat.class.declass">
        <xsl:with-param name="warning-missing-classif" select="string($warning-missing-classif)"/>
        <xsl:with-param name="warning-unclass-and-classified-markings"
          select="string($warning-unclass-and-classified-markings)"/>
        <xsl:with-param name="warning-missing-classifiedBy"
          select="string($warning-missing-classifiedBy)"/>
        <xsl:with-param name="warning-both-original-and-derivatively-classified"
          select="string($warning-both-original-and-derivatively-classified)"/>
        <xsl:with-param name="warning-missing-classificationReason"
          select="string($warning-missing-classificationReason)"/>
        <xsl:with-param name="warning-missing-declass-instructions"
          select="string($warning-missing-declass-instructions)"/>
        <xsl:with-param name="warning-missing-declass-info"
          select="string($warning-missing-declass-info)"/>
        <xsl:with-param name="warning-invalid-declass-date-and-exemption"
          select="string($warning-invalid-declass-date-and-exemption)"/>
        <xsl:with-param name="warning-invalid-declass-event-and-exemption"
          select="string($warning-invalid-declass-event-and-exemption)"/>
        <xsl:with-param name="warning-missing-cuiControlledBy"
          select="string($warning-missing-cuiControlledBy)"/>
        <xsl:with-param name="classified-by-line" select="string($classified-by-line)"/>
        <xsl:with-param name="derivatively-classified-by-line"
          select="string($derivatively-classified-by-line)"/>
        <xsl:with-param name="derived-from-line" select="string($derived-from-line)"/>
        <xsl:with-param name="reason-line" select="string($reason-line)"/>
        <xsl:with-param name="declassify-on-line" select="string($declassify-on-line)"/>
        <xsl:with-param name="cui-controlled-by-line" select="string($cui-controlled-by-line)"/>
        <xsl:with-param name="cui-decontrol-on-line" select="string($cui-decontrol-on-line)"/>
        <xsl:with-param name="cui-controlled-by-office-line"
          select="string($cui-controlled-by-office-line)"/>
        <xsl:with-param name="cui-POC-line" select="string($cui-POC-line)"/>
        <xsl:with-param name="cui-categories-line"
          select="$cui-categories-line"/>
        <xsl:with-param name="dissemVal" select="$dissemVal"/>
        <xsl:with-param name="delimiter" select="$class-declass-delimiter"/>
      </xsl:call-template>
    </xsl:if>
  </xsl:template>

  <xsl:template name="ism:concat.class.declass">
    <!-- **************************************************************** -->
    <!-- concat.class.declass - Concatenates class/declass block content  -->
    <!--                        into a delimited string.  Generates CUI   -->
    <!--                        Control Block if there are CUI markings.  -->
    <!-- **************************************************************** -->
    <xsl:param name="warning-missing-classif"/>
    <xsl:param name="warning-unclass-and-classified-markings"/>
    <xsl:param name="warning-missing-classifiedBy"/>
    <xsl:param name="warning-both-original-and-derivatively-classified"/>
    <xsl:param name="warning-missing-classificationReason"/>
    <xsl:param name="warning-missing-declass-instructions"/>
    <xsl:param name="warning-missing-declass-info"/>
    <xsl:param name="warning-invalid-declass-date-and-exemption"/>
    <xsl:param name="warning-invalid-declass-event-and-exemption"/>
    <xsl:param name="warning-missing-cuiControlledBy"/>
    <xsl:param name="classified-by-line"/>
    <xsl:param name="derivatively-classified-by-line"/>
    <xsl:param name="derived-from-line"/>
    <xsl:param name="reason-line"/>
    <xsl:param name="declassify-on-line"/>
    <xsl:param name="cui-controlled-by-line"/>
    <xsl:param name="cui-decontrol-on-line"/>
    <xsl:param name="cui-controlled-by-office-line"/>
    <xsl:param name="cui-POC-line"/>
    <xsl:param name="cui-categories-line"/>
    <xsl:param name="dissemVal"/>
    <xsl:param name="delimiter"/>

    <xsl:variable name="class-declass-content">
      <xsl:if test="$warning-missing-classif">
        <xsl:value-of select="$delimiter"/>
        <xsl:value-of select="$warning-missing-classif"/>
      </xsl:if>
      <xsl:if test="$warning-unclass-and-classified-markings">
        <xsl:value-of select="$delimiter"/>
        <xsl:value-of select="$warning-unclass-and-classified-markings"/>
      </xsl:if>
      <xsl:if test="$warning-missing-classifiedBy">
        <xsl:value-of select="$delimiter"/>
        <xsl:value-of select="$warning-missing-classifiedBy"/>
      </xsl:if>

      <!-- **************************************************************** -->
      <!-- Include the "Classified by" line or the "Derived from" line.     -->
      <!--                                                                  -->
      <!-- NOTE: A classified document can be either an originally          -->
      <!--       classified document or a derivatively classified document. -->
      <!--       An originally classified document will always contain a    -->
      <!--       "Classified by" line.  A derivatively classified document  -->
      <!--       may (somewhat misleadingly) contain a "Classified by" line -->
      <!--       if attribute @derivativelyClassifiedBy exists, and will    -->
      <!--       always contain a "Derived from" line.                      -->
      <!-- **************************************************************** -->
      <xsl:if test="$classified-by-line or $derived-from-line">
        <xsl:value-of select="$delimiter"/>
        <xsl:choose>
          <xsl:when test="$classified-by-line">
            <xsl:text>Classified by: </xsl:text>
            <xsl:value-of select="$classified-by-line"/>
          </xsl:when>
          <xsl:when test="$derived-from-line">
            <xsl:if test="$derivatively-classified-by-line">
              <xsl:text>Classified by: </xsl:text>
              <xsl:value-of select="$derivatively-classified-by-line"/>
              <xsl:value-of select="$delimiter"/>
            </xsl:if>
            <xsl:text>Derived from: </xsl:text>
            <xsl:value-of select="$derived-from-line"/>
          </xsl:when>
        </xsl:choose>
      </xsl:if>

      <!-- **************************************************************** -->

      <!-- **************************************************************** -->
      <!-- Include the "Reason" line.                                       -->
      <!--                                                                  -->
      <!-- NOTE: For originally classified documents, the reason for the    -->
      <!--       classification decision should always be specified.        -->
      <!--                                                                  -->
      <!--       According to ISOO Directive 1, Section 2001.22(c), for     -->
      <!--       derivatively classified documents, the reason for the      -->
      <!--       original classification decision, as reflected in the      -->
      <!--       source document(s) or classification guide, is not         -->
      <!--       required.  If included, however, it shall conform to the   -->
      <!--       standards in Section 2001.21(a)(3).                        -->
      <!--                                                                  -->
      <!--       In other words, the "Reason" line can be included in the   -->
      <!--       classification/declassification block for derivatively     -->
      <!--       classified documents.                                      -->
      <!-- **************************************************************** -->
      <xsl:if test="$reason-line">
        <xsl:value-of select="$delimiter"/>
        <xsl:text>Reason: </xsl:text>
        <xsl:value-of select="$reason-line"/>
      </xsl:if>

      <!-- **************************************************************** -->
      <xsl:if test="$warning-both-original-and-derivatively-classified">
        <xsl:value-of select="$delimiter"/>
        <xsl:value-of select="$warning-both-original-and-derivatively-classified"/>
      </xsl:if>
      <xsl:if test="$warning-missing-classificationReason">
        <xsl:value-of select="$delimiter"/>
        <xsl:value-of select="$warning-missing-classificationReason"/>
      </xsl:if>
      <xsl:if test="$warning-missing-declass-instructions">
        <xsl:value-of select="$delimiter"/>
        <xsl:value-of select="$warning-missing-declass-instructions"/>
      </xsl:if>

      <!-- **************************************************************** -->
      <!-- Include the "Declassify on" line.                                -->
      <!-- **************************************************************** -->
      <xsl:if test="$declassify-on-line">
        <xsl:value-of select="$delimiter"/>
        <xsl:text>Declassify on: </xsl:text>
        <xsl:value-of select="$declassify-on-line"/>
      </xsl:if>

      <!-- **************************************************************** -->
      <xsl:if test="$warning-missing-declass-info">
        <xsl:value-of select="$delimiter"/>
        <xsl:value-of select="$warning-missing-declass-info"/>
      </xsl:if>
      <xsl:if test="$warning-invalid-declass-date-and-exemption">
        <xsl:value-of select="$delimiter"/>
        <xsl:value-of select="$warning-invalid-declass-date-and-exemption"/>
      </xsl:if>
      <xsl:if test="$warning-invalid-declass-event-and-exemption">
        <xsl:value-of select="$delimiter"/>
        <xsl:value-of select="$warning-invalid-declass-event-and-exemption"/>
      </xsl:if>
    </xsl:variable>

    <xsl:value-of select="substring-after($class-declass-content, $delimiter)"/>

    <!-- **************************************************************** -->
    <!--    Output CUI block information                                  -->
    <!-- **************************************************************** -->

    <xsl:variable name="cui-content">
      <xsl:if test="$warning-missing-cuiControlledBy">
        <xsl:value-of select="$delimiter"/>
        <xsl:value-of select="$warning-missing-cuiControlledBy"/>
      </xsl:if>

      <xsl:if test="$cui-controlled-by-line">
        <xsl:value-of select="$delimiter"/>
        <xsl:text>Controlled By: </xsl:text>
        <xsl:value-of select="$cui-controlled-by-line"/>
      </xsl:if>

      <xsl:if test="$cui-controlled-by-office-line">
        <xsl:value-of select="$delimiter"/>
        <xsl:text>Controlled By: </xsl:text>
        <xsl:value-of select="$cui-controlled-by-office-line"/>
      </xsl:if>

      <xsl:if test="$CUIRenderingRuleSet = 'DOD'">
        <xsl:value-of select="$delimiter"/>
        <xsl:text>CUI Category: </xsl:text>
        <xsl:value-of select="$cui-categories-line"/>
        <xsl:if test="$dissemVal != ''">
          <xsl:value-of select="$delimiter"/>
          <xsl:text>Distribution/Dissemination Control: </xsl:text>
          <xsl:value-of select="replace($dissemVal, '/', '; ')"/>
        </xsl:if>
      </xsl:if>

      <xsl:if test="$cui-POC-line">
        <xsl:value-of select="$delimiter"/>
        <xsl:text>POC: </xsl:text>
        <xsl:value-of select="$cui-POC-line"/>
      </xsl:if>

      <xsl:if test="$cui-decontrol-on-line">
        <xsl:value-of select="$delimiter"/>
        <xsl:text>Decontrol On: </xsl:text>
        <xsl:value-of select="$cui-decontrol-on-line"/>
      </xsl:if>

    </xsl:variable>
    
    <!-- Output CUI Control Block, with delimiter if document is classified otherwise no delimiter -->
    <xsl:choose>
      <xsl:when test="$classified-by-line or $derived-from-line">
        <xsl:value-of select="$cui-content"/>
      </xsl:when>
      <xsl:otherwise>
        <xsl:value-of select="substring-after($cui-content, $delimiter)"/>
      </xsl:otherwise>
    </xsl:choose>
    
  </xsl:template>


</xsl:stylesheet>
<!-- **************************************************************** -->
<!--                          CHANGE HISTORY                          -->
<!--                                                                  -->
<!-- Version 1.0                                                      -->
<!--                                                                  -->
<!--   - Unpublished                                                  -->
<!--                                                                  -->
<!-- Version 2.0                                                      -->
<!--                                                                  -->
<!--   - Initial published version corresponding to IC ISM v2.0       -->
<!--                                                                  -->
<!-- Version 2.0.1                                                    -->
<!--                                                                  -->
<!--   - Modified to separate the rendering of content as HTML into   -->
<!--     an independent template                                      -->
<!--                                                                  -->
<!--   - Other minor modifications                                    -->
<!--                                                                  -->
<!-- Version 2.0.2                                                    -->
<!--                                                                  -->
<!--   - Modified so that content is not rendered into any specific   -->
<!--     format by this stylesheet.  Instead the content is simply    -->
<!--     concatenated into a delimited text string, where each        -->
<!--     section of the delimited string is a line of content within  -->
<!--     the class/declass block, including various warning messages. -->
<!--     This method leaves it up to the calling stylesheet to parse  -->
<!--     the delimited string and render it according to the desired  -->
<!--     output.  A delimiter can be set in the calling stylesheet    -->
<!--     and passed into the class.declass template as a parameter by -->
<!--     the template call in the calling stylesheet.  If a delimiter -->
<!--     is not set and/or not passed in, the delimiter is set to a   -->
<!--     "|" in the class.declass template in this stylesheet, and    -->
<!--     the calling stylesheet MUST parse the string based on this   -->
<!--     delimiter.                                                   -->
<!--                                                                  -->
<!-- Version 2.1                                                      -->
<!--                                                                  -->
<!--   - Corresponds to IC ISM 2.1 (ISM-XML 1.0).                     -->
<!--                                                                  -->
<!--   - Modified the "Declassify on" line (declassify-on-line) to    -->
<!--     exclude the optional time zone indicator which may exist in  -->
<!--     the @declassDate and @dateOfExemptedSource attribute values. -->
<!--                                                                  -->
<!--   - Added a template to convert date values from "YYYY-MM-DD"    -->
<!--     format to "Month [D]D, YYYY" format for the @declassDate and -->
<!--     @dateOfExemptedSource attributes.                            -->
<!--                                                                  -->
<!--   - Added a recursive template to include a space after each     -->
<!--     comma when multiple declassification exemption markings or   -->
<!--     multiple type of exempted source markings are specified.     -->
<!--                                                                  -->
<!--   - Added a warning when a 25X declassification exemption (other -->
<!--     than 25X1-human) is specified and a declassification date or -->
<!--     declassification event is not specified.                     -->
<!--                                                                  -->
<!--   - Modified stylesheet to account for @derivativelyClassifiedBy -->
<!--     attribute.                                                   -->
<!--                                                                  -->
<!-- 2010-09-24                                                   
  - Changed the name of warning variables to be more descriptive.
  - Namespace qualified templates, except for class.declass and get.class.declass.  
-->
<!-- 2011-01-28                                                   
  - Added convenience template with mode="ism:authority" for processing current element to generate Classification Authority Block
  - Changed namespace for qualified templates to use ISM namespace, except for class.declass and get.class.declass (preserved for compatibility).  
-->
<!-- 2011-08-12
  - Changed logic for declass exception warnings to include 25X1-HUM and 25X2-WMD
-->
<!-- 2013-01-02
    Removed NU since it is no longer a classification.
    Added logic for NATO and NATO-AEA as a new exemptions.

-->
<!-- **************************************************************** -->
<!-- **************************************************************** -->
<!--                            UNCLASSIFIED                          -->
<!-- **************************************************************** -->
