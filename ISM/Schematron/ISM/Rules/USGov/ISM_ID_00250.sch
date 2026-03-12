<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00250">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00250][Error] If ISM_USGOV_RESOURCE, element ism:Notice must specify 
		attribute @ism:noticeType or @ism:unregisteredNoticeType.
		
		Human Readable: Notices must specify their type.
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		This rule ensures for element ism:Notice must specify their type.
	</sch:p>
	  <sch:rule id="ISM-ID-00250-R1" context="ism:Notice[$ISM_USGOV_RESOURCE]">
		    <sch:assert test="@ism:noticeType or @ism:unregisteredNoticeType" flag="error" role="error">
		    	[ISM-ID-00250][Error] If ISM_USGOV_RESOURCE, element ism:Notice must specify 
		    	attribute @ism:noticeType or @ism:unregisteredNoticeType.
		    	
		    	Human Readable: Notices must specify their type.
		</sch:assert>
	  </sch:rule>
</sch:pattern>