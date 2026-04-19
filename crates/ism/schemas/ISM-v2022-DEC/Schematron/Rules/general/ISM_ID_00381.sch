<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" xmlns:ism="urn:us:gov:ic:ism" id="ISM-ID-00381">
	  <sch:p ism:classification="U" ism:ownerProducer="USA" class="ruleText">
	  	[ISM-ID-00381][Error] 
	  	1. @ism:compliesWith of resource element contains USIC or USDOD
	  	2. @ism:compliesWith must also contain USGov
	</sch:p>
	<sch:p ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		If the resource element contains @ism:compliesWith of either USIC or USDOD, then @ism:compliesWith must also contain USGov.
	</sch:p>
	<sch:rule id="ISM-ID-00381-R1" context="*[ generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and        util:containsAnyOfTheTokens(@ism:compliesWith, ('USIC','USDOD'))]">
		<sch:assert test="util:containsAnyOfTheTokens(@ism:compliesWith, ('USGov'))" flag="error" role="error"> 
			[ISM-ID-00381][Error] 
			1. @ism:compliesWith of resource element contains USIC or USDOD
			2. @ism:compliesWith must also contain USGov
			
			Human Readable: All documents that contain USIC or USDOD in @ism:compliesWith of
			the first resource node (in document order) must also contain USGov in @ism:compliesWith
		</sch:assert>
	</sch:rule>
</sch:pattern>