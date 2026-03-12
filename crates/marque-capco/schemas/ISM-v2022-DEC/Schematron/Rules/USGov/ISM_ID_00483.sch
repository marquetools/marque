<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00483" is-a="AttributeValueDeprecatedError">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    	[ISM-ID-00483][Error] Attribute @ism:cuiSpecified must not contain values which have passed their deprecation date.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      For each element which specifies attribute @ism:cuiSpecified,
      this rule ensures that the value of @ism:cuiSpecified has not been deprecated. 
      This is indicated in the CVE file by an attribute (@deprecated) 
      on the term element for that cuiSpecified value. 
      If the current date is greater than the date value in (@deprecated), 
      then a deprecation error will be given.
    </sch:p>
    <sch:param name="ruleId" value="'ISM-ID-00483'"/>
    <sch:param name="context" value="*[@ism:cuiSpecified]"/>
    <sch:param name="attrName" value="cuiSpecified"/>
    <sch:param name="cveName" value="CVEnumISMCUISpecified"/>
    <sch:param name="cveSpec" value="ISM"/>
</sch:pattern>