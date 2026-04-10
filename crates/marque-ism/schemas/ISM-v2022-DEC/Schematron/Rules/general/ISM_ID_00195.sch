<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00195" is-a="AttributeValueDeprecatedError">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    	[ISM-ID-00195][Error] Attribute @ism:noticeType must NOT contain values which have passed their deprecation date.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      For each element which specifies attribute @ism:noticeType, this rule ensures that the value of @ism:noticeType has not been deprecated. 
      This is indicated in the CVE file by an attribute (@deprecated) on the term element for that noticeType value. 
      If the current date is greater than the date value in (@deprecated), then a deprecation error will be given.</sch:p>
	  <sch:param name="ruleId" value="'ISM-ID-00195'"/>
	  <sch:param name="context" value="*[@ism:noticeType]"/>
	  <sch:param name="attrName" value="noticeType"/>
	  <sch:param name="cveName" value="CVEnumISMNotice"/>
	  <sch:param name="cveSpec" value="ISM"/>
</sch:pattern>