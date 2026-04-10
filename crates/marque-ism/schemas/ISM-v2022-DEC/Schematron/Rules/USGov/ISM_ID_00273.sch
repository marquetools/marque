<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00273">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00273][Error] All @ism:exemptFrom attributes must be of type NmTokens. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:exemptFrom attribute, this rule ensures that the exemptFrom value matches the pattern
		defined for type NmTokens. 
	</sch:p>
	  <sch:rule id="ISM-ID-00273-R1" context="*[@ism:exemptFrom]">
		    <sch:assert test="util:meetsType(@ism:exemptFrom, $NmTokensPattern)" flag="error" role="error">
		    	[ISM-ID-00273][Error] All @ism:exemptFrom attributes must be of type NmTokens. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>