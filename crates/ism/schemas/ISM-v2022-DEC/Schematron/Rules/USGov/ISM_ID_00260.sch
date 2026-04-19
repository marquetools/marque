<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00260" is-a="ValidateTokenValuesExistenceInList">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    	[ISM-ID-00260][Error] All @ism:FGIsourceProtected values must be defined in CVEnumISMCATFGIProtected.xml.
    </sch:p>
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		This rule uses an abstract pattern to consolidate logic. It checks that the
		value in parameter $searchTerm is contained in the parameter $list. The parameter
		$searchTerm is relative in scope to the parameter $context. The value for the parameter 
		$list is a variable defined in the main document (ISM_XML.sch), which reads 
		values from a specific CVE file.
	</sch:p>
	<sch:param name="context" value="*[@ism:FGIsourceProtected]"/>
	<sch:param name="searchTermList" value="@ism:FGIsourceProtected"/>
	<sch:param name="list" value="$FGIsourceProtectedList"/>
	<sch:param name="errMsg" value="'   [ISM-ID-00260][Error] All @ism:FGIsourceProtected values must   be defined in CVEnumISMCATFGIProtected.xml.   '"/>
</sch:pattern>